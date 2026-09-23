use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("API Token 无效或缺失或已撤销")]
    InvalidToken,
    #[error("API Token 已被禁用")]
    Disabled,
    #[error("API Token 已过期")]
    Expired,
    #[error("API Token 已撤销，不可恢复")]
    Revoked,
    #[error("参数无效")]
    Input,
    #[error("记录不存在")]
    NotFound,
    #[error("请求过于频繁，请稍后重试")]
    Limited,
    #[error("需要管理员认证")]
    Admin,
    #[error("不允许的跨站请求")]
    Origin,
    #[error("存储服务错误")]
    Storage,
}
impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidToken => "invalid_token",
            Self::Disabled => "token_disabled",
            Self::Expired => "token_expired",
            Self::Revoked => "token_revoked",
            Self::Input => "invalid_input",
            Self::NotFound => "not_found",
            Self::Limited => "rate_limited",
            Self::Admin => "admin_required",
            Self::Origin => "origin_denied",
            Self::Storage => "storage_error",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::InvalidToken | Self::Admin => StatusCode::UNAUTHORIZED,
            Self::Disabled | Self::Expired | Self::Revoked | Self::Origin => StatusCode::FORBIDDEN,
            Self::Input => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Limited => StatusCode::TOO_MANY_REQUESTS,
            Self::Storage => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let limited = matches!(self, Self::Limited);
        let mut response = (
            self.status(),
            Json(json!({"code":self.code(),"message":self.to_string()})),
        )
            .into_response();
        if limited {
            response
                .headers_mut()
                .insert("Retry-After", "60".parse().unwrap());
        }
        response
    }
}
