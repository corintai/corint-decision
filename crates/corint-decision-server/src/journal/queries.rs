//! Closed set of journal data operations. Storage binds $1 to its immutable tenant
//! before returning a query; callers can supply only operation-specific values.
#[derive(Clone, Copy)]
pub(super) enum Operation {
    Status,
    Usage,
    LockUsage,
    ReservationOwned,
    Decision,
    Response,
    InsertDecision,
    SaveResponse,
    CompleteRequest,
    Request,
    DeleteExpired,
    Pending,
    Reserve,
    Abandon,
    Claim,
    Lease,
    Acknowledge,
}

impl Operation {
    pub(super) fn sql(self, postgres: bool) -> &'static str {
        match self {
            Self::Status => "SELECT record_count,total_bytes,(SELECT COUNT(*) FROM request_keys WHERE tenant_id=$1 AND decision_id IS NULL AND expires>$2) AS pending FROM journal_usage WHERE tenant_id=$1 AND id=1",
            Self::Usage => "SELECT record_count,total_bytes FROM journal_usage WHERE tenant_id=$1 AND id=1",
            Self::LockUsage => "SELECT id FROM journal_usage WHERE tenant_id=$1 AND id=1 FOR UPDATE",
            Self::ReservationOwned => "SELECT COUNT(*) AS count FROM request_keys WHERE tenant_id=$1 AND key=$2 AND owner=$3 AND decision_id IS NULL",
            Self::Decision if postgres => "SELECT digest,input FROM events WHERE tenant_id=$1 AND kind='decision-record' AND (body::jsonb->>'tenant_id')=$1 AND (body::jsonb->>'decision_id')=$2",
            Self::Decision => "SELECT digest,input FROM events WHERE tenant_id=$1 AND kind='decision-record' AND json_extract(body,'$.tenant_id')=$1 AND json_extract(body,'$.decision_id')=$2",
            Self::Response if postgres => "SELECT http_status,response FROM events WHERE tenant_id=$1 AND kind='decision-record' AND (body::jsonb->>'tenant_id')=$1 AND (body::jsonb->>'decision_id')=$2",
            Self::Response => "SELECT http_status,response FROM events WHERE tenant_id=$1 AND kind='decision-record' AND json_extract(body,'$.tenant_id')=$1 AND json_extract(body,'$.decision_id')=$2",
            Self::InsertDecision => "INSERT INTO events(tenant_id,kind,digest,body,input,bytes) VALUES($1,$2,$3,$4,$5,$6)",
            Self::SaveResponse => "UPDATE events SET response=$2,http_status=$3 WHERE tenant_id=$1 AND digest=$4",
            Self::CompleteRequest => "UPDATE request_keys SET decision_id=$2,expires=0 WHERE tenant_id=$1 AND key=$3 AND owner=$4",
            Self::Request => "SELECT fingerprint,owner,decision_id,expires FROM request_keys WHERE tenant_id=$1 AND key=$2",
            Self::DeleteExpired => "DELETE FROM request_keys WHERE tenant_id=$1 AND decision_id IS NULL AND expires<=$2",
            Self::Pending => "SELECT COUNT(*) AS count FROM request_keys WHERE tenant_id=$1 AND decision_id IS NULL",
            Self::Reserve => "INSERT INTO request_keys(tenant_id,key,fingerprint,owner,expires) VALUES($1,$2,$3,$4,$5)",
            Self::Abandon => "DELETE FROM request_keys WHERE tenant_id=$1 AND key=$2 AND owner=$3 AND decision_id IS NULL",
            Self::Claim => "SELECT seq,digest,body,input,response,bytes,attempts FROM events WHERE tenant_id=$1 AND kind='decision-record' AND delivered=0 AND retry_at<=$2 ORDER BY seq LIMIT 100",
            Self::Lease => "UPDATE events SET lease=$2,retry_at=$3,attempts=attempts+1 WHERE tenant_id=$1 AND seq=$4",
            Self::Acknowledge => "UPDATE events SET delivered=1 WHERE tenant_id=$1 AND digest=$2 AND lease=$3 AND retry_at>$4",
        }
    }
}
