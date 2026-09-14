//! Fixed, scoped SQL operations. Tenant parameters are bound before a caller
//! receives the query. Schema setup is kept in separate operator-only code.
use super::Scope;
use sqlx::{any::AnyArguments, query::Query, Any};

#[derive(Clone, Copy)]
pub(crate) enum Operation {
    State,
    Pause,
    Audit,
    InsertAudit,
    SqlitePublication,
    PostgresPublication,
}
impl Operation {
    fn sql(self) -> &'static str {
        match self {
            Self::State => "SELECT paused,revision FROM tenant_runtimes WHERE tenant_id=$1 AND scope_key=$2",
            Self::Pause => "UPDATE tenant_runtimes SET paused=$3,revision=revision+1 WHERE tenant_id=$1 AND scope_key=$2 AND revision=$4",
            Self::Audit => "SELECT event FROM tenant_management_audit WHERE tenant_id=$1 AND scope_key=$2 ORDER BY revision DESC LIMIT 100",
            Self::InsertAudit => "INSERT INTO tenant_management_audit(tenant_id,scope_key,revision,event) VALUES($1,$2,$3,$4)",
            Self::SqlitePublication => "SELECT document FROM corint_tenant_publication WHERE tenant_id=$1 AND environment=$2 AND deployment=$3 AND length(CAST(document AS BLOB))<=33554432",
            Self::PostgresPublication => "SELECT document FROM corint_tenant_publication WHERE tenant_id=$1 AND environment=$2 AND deployment=$3 AND octet_length(document)<=33554432",
        }
    }
}

pub(crate) fn query<'q>(
    scope: &Scope,
    op: Operation,
) -> anyhow::Result<Query<'q, Any, AnyArguments<'q>>> {
    scope.validate()?;
    let query = sqlx::query(op.sql()).bind(scope.tenant_id.clone());
    Ok(match op {
        Operation::SqlitePublication | Operation::PostgresPublication => query
            .bind(scope.environment.clone())
            .bind(scope.deployment.clone()),
        _ => query.bind(scope.key()),
    })
}

// Repository connections retain their concrete backend types. These helpers
// bind exactly the same validated identity as the Any control-store queries.
pub(crate) fn sqlite_publication<'q>(
    scope: &Scope,
) -> anyhow::Result<Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>> {
    scope.validate()?;
    Ok(sqlx::query(Operation::SqlitePublication.sql())
        .bind(scope.tenant_id.clone())
        .bind(scope.environment.clone())
        .bind(scope.deployment.clone()))
}
pub(crate) fn postgres_publication<'q>(
    scope: &Scope,
) -> anyhow::Result<Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>> {
    scope.validate()?;
    Ok(sqlx::query(Operation::PostgresPublication.sql())
        .bind(scope.tenant_id.clone())
        .bind(scope.environment.clone())
        .bind(scope.deployment.clone()))
}
