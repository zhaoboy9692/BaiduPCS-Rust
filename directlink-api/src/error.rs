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
    #[error("Token 提取次数已用尽，请联系管理员增加额度")]
    QuotaExhausted,
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
    #[error("原服务不可用或响应异常，请检查原后台登录状态")]
    Upstream,
    #[error("分享需要提取码")]
    PasswordRequired,
    #[error("分享提取码错误")]
    PasswordInvalid,
    #[error("分享已失效或不存在")]
    ShareUnavailable,
    #[error("所选文件未在分享中找到，请刷新文件列表重新选择")]
    SelectionNotFound,
    #[error("文件夹内容读取失败，未能完整遍历该目录，请重试")]
    DirectoryFailed,
    #[error("正在处理其他提取请求，请稍后重试")]
    Busy,
    #[error("等待超时，转存可能仍在执行；请到原转存任务页面检查，勿立即重复提交")]
    Timeout,
    #[error("转存失败，请在原任务页面查看原因")]
    TransferFailed,
    #[error("存储服务错误")]
    Storage,
}
impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidToken => "invalid_token",
            Self::Disabled => "token_disabled",
            Self::Expired => "token_expired",
            Self::QuotaExhausted => "token_quota_exhausted",
            Self::Revoked => "token_revoked",
            Self::Input => "invalid_input",
            Self::NotFound => "not_found",
            Self::Limited => "rate_limited",
            Self::Admin => "admin_required",
            Self::Origin => "origin_denied",
            Self::Storage => "storage_error",
            Self::Upstream => "upstream_unavailable",
            Self::PasswordRequired => "share_password_required",
            Self::PasswordInvalid => "share_password_invalid",
            Self::ShareUnavailable => "share_unavailable",
            Self::SelectionNotFound => "selection_not_found",
            Self::DirectoryFailed => "directory_failed",
            Self::Busy => "resolver_busy",
            Self::Timeout => "resolve_timeout",
            Self::TransferFailed => "transfer_failed",
        }
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::InvalidToken | Self::Admin => StatusCode::UNAUTHORIZED,
            Self::Disabled
            | Self::Expired
            | Self::Revoked
            | Self::Origin
            | Self::QuotaExhausted => StatusCode::FORBIDDEN,
            Self::Input => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Limited => StatusCode::TOO_MANY_REQUESTS,
            Self::Storage => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Upstream | Self::Busy => StatusCode::SERVICE_UNAVAILABLE,
            Self::PasswordRequired | Self::PasswordInvalid | Self::ShareUnavailable => {
                StatusCode::BAD_REQUEST
            }
            Self::SelectionNotFound | Self::DirectoryFailed => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
            Self::TransferFailed => StatusCode::BAD_GATEWAY,
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
