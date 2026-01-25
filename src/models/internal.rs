use sqlx::FromRow;

#[derive(Clone, Debug)]
pub struct Station {
    pub id: String,
    pub name: String,
    pub provider: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct Lio {
   pub id: String,
   pub provider: String,
   pub provider_id: String,
   pub station: String,
   pub line: String,
   pub direction: String,
}
