//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.3

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "referee")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(column_type = "Decimal(Some((20, 10)))")]
    pub one_time_bonus_applied_for_referee: Decimal,
    #[sea_orm(column_type = "Decimal(Some((20, 10)))")]
    pub credits_applied_for_referrer: Decimal,
    pub referral_start_date: DateTime,
    pub used_referral_code: i32,
    #[sea_orm(unique)]
    pub user_id: u64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::referrer::Entity",
        from = "Column::UsedReferralCode",
        to = "super::referrer::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Referrer,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    User,
}

impl Related<super::referrer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Referrer.def()
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
