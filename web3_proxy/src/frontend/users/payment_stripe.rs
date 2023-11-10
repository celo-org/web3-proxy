use crate::app::App;
use crate::errors::{Web3ProxyError, Web3ProxyErrorContext, Web3ProxyResponse};
use crate::globals::global_db_conn;
use crate::premium::grant_premium_tier;
use anyhow::Context;
use axum::{extract::State, response::IntoResponse};
use axum_macros::debug_handler;
use entities::{stripe_increase_balance_receipt, user, user_tier};
use http::HeaderMap;
use migration::sea_orm::prelude::Decimal;
use migration::sea_orm::{
    self, ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
};
use std::sync::Arc;
use stripe::Webhook;
use tracing::{debug, error, warn};

/// `POST /user/balance/stripe` -- Process a stripe transaction;
/// this endpoint is called from the webhook with the user_id parameter in the request
#[debug_handler]
pub async fn user_balance_stripe_post(
    State(app): State<Arc<App>>,
    // InsecureClientIp(ip): InsecureClientIp,
    headers: HeaderMap,
    payload: String,
) -> Web3ProxyResponse {
    // TODO: (high) rate limits by IP address. login limiter is probably too low
    // TODO: maybe instead, a bad stripe-header should ban the IP? or a good one should allow it?

    // TODO: lower log level when done testing
    debug!(%payload, ?headers);

    // get the signature from the header
    // the docs are inconsistent on the key, so we just check all of them
    let signature = if let Some(x) = headers.get("stripe-signature") {
        x
    } else if let Some(x) = headers.get("Stripe-Signature") {
        x
    } else if let Some(x) = headers.get("STRIPE_SIGNATURE") {
        x
    } else if let Some(x) = headers.get("HTTP_STRIPE_SIGNATURE") {
        x
    } else {
        return Err(Web3ProxyError::BadRequest(
            "You have not provided a 'STRIPE_SIGNATURE' for the Stripe payload".into(),
        ));
    };

    let signature = signature
        .to_str()
        .web3_context("Could not parse stripe signature as byte-string")?;

    let secret = app
        .config
        .stripe_whsec_key
        .clone()
        .web3_context("Stripe API key not found in config!")?;

    let event = Webhook::construct_event(&payload, signature, secret.as_str())?;

    let intent = match event.data.object {
        stripe::EventObject::PaymentIntent(intent) => intent,
        _ => return Ok("Received irrelevant webhook".into_response()),
    };

    debug!(?intent);

    if intent.status.as_str() != "succeeded" {
        return Ok("Received Webhook".into_response());
    }

    let db_conn = global_db_conn().web3_context("query_user_stats needs a db")?;

    if stripe_increase_balance_receipt::Entity::find()
        .filter(
            stripe_increase_balance_receipt::Column::StripePaymentIntendId.eq(intent.id.as_str()),
        )
        .one(&db_conn)
        .await?
        .is_some()
    {
        return Ok("Payment was already recorded".into_response());
    };

    // Try to get the recipient_user_id from the data metadata
    let recipient_user_id = match intent.metadata.get("user_id") {
        Some(x) => Ok(x.parse::<u64>()),
        None => Err(Web3ProxyError::BadRequest(
            "Could not find user_id in the stripe webhook request!".into(),
        )),
    }?
    .context(Web3ProxyError::BadRequest(
        "Could not parse the stripe webhook request user_id!".into(),
    ))?;

    let recipient: Option<user::Model> = user::Entity::find_by_id(recipient_user_id)
        .one(&db_conn)
        .await?;

    // we do a fixed 2 decimal points because we only accept USD for now
    let amount = Decimal::new(intent.amount, 2);
    let recipient_id: Option<u64> = recipient.as_ref().map(|x| x.id);
    let insert_receipt_model = stripe_increase_balance_receipt::ActiveModel {
        id: Default::default(),
        deposit_to_user_id: sea_orm::Set(recipient_id),
        amount: sea_orm::Set(amount),
        stripe_payment_intend_id: sea_orm::Set(intent.id.as_str().to_string()),
        currency: sea_orm::Set(intent.currency.to_string()),
        status: sea_orm::Set(intent.status.to_string()),
        description: sea_orm::Set(intent.description),
        date_created: Default::default(),
    };

    // In all these cases, we should record the transaction, but not increase the balance

    // Assert that it's usd
    if intent.currency.to_string() != "usd" || recipient.is_none() {
        // In this case I should probably still save it to the database,
        // but not increase balance (this should be refunded)
        // TODO: I suppose we could send a refund request right away from here
        error!(
            currency=%intent.currency, %recipient_user_id, %intent.id,
            "Please refund this transaction!",
        );
        let _ = insert_receipt_model.save(&db_conn).await;

        return Ok("Received Webhook".into_response());
    }
    // Otherwise, also increase the balance ...
    match recipient {
        Some(recipient) => {
            let txn = db_conn.begin().await?;

            let _ = insert_receipt_model.save(&txn).await;

            let user_tier = user_tier::Entity::find_by_id(recipient.user_tier_id)
                .one(&txn)
                .await?;

            grant_premium_tier(&recipient, user_tier.as_ref(), &txn)
                .await
                .web3_context("granting premium tier")?;

            txn.commit().await?;

            // Finally invalidate the cache as well
            if let Err(err) = app
                .user_balance_cache
                .invalidate(&recipient.id, &db_conn, &app.rpc_secret_key_cache)
                .await
            {
                warn!(?err, "unable to invalidate caches");
            };
        }
        None => {
            return Err(Web3ProxyError::BadResponse(
                "We just checked if the recipient is not none, it should've branched before!"
                    .into(),
            ))
        }
    };

    Ok("Received webhook".into_response())
}
