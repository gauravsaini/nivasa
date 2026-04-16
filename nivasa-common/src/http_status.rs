//! Typed HTTP status codes for the Nivasa framework.
//!
//! `HttpStatus` gives you a typed wrapper around standard HTTP status codes,
//! with helpers for numeric conversion, `http::StatusCode`, and
//! `HttpException`.
//!
//! # Example
//!
//! ```rust
//! use nivasa_common::HttpStatus;
//!
//! let status = HttpStatus::NotFound;
//!
//! assert_eq!(status.as_u16(), 404);
//! assert_eq!(status.reason_phrase(), "Not Found");
//! assert!(status.is_client_error());
//! ```

use std::fmt;

use http::StatusCode;
use thiserror::Error;

use crate::HttpException;

/// A standard HTTP status code.
///
/// Use this enum when you want typed status handling instead of raw numbers.
///
/// ```rust
/// use nivasa_common::HttpStatus;
///
/// let status = HttpStatus::Created;
/// assert_eq!(u16::from(status), 201);
/// assert_eq!(status.to_string(), "201 Created");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u16)]
pub enum HttpStatus {
    Continue = 100,
    SwitchingProtocols = 101,
    Processing = 102,
    EarlyHints = 103,
    Ok = 200,
    Created = 201,
    Accepted = 202,
    NonAuthoritativeInformation = 203,
    NoContent = 204,
    ResetContent = 205,
    PartialContent = 206,
    MultiStatus = 207,
    AlreadyReported = 208,
    ImUsed = 226,
    MultipleChoices = 300,
    MovedPermanently = 301,
    Found = 302,
    SeeOther = 303,
    NotModified = 304,
    UseProxy = 305,
    TemporaryRedirect = 307,
    PermanentRedirect = 308,
    BadRequest = 400,
    Unauthorized = 401,
    PaymentRequired = 402,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    NotAcceptable = 406,
    ProxyAuthenticationRequired = 407,
    RequestTimeout = 408,
    Conflict = 409,
    Gone = 410,
    LengthRequired = 411,
    PreconditionFailed = 412,
    PayloadTooLarge = 413,
    UriTooLong = 414,
    UnsupportedMediaType = 415,
    RangeNotSatisfiable = 416,
    ExpectationFailed = 417,
    ImATeapot = 418,
    MisdirectedRequest = 421,
    UnprocessableEntity = 422,
    Locked = 423,
    FailedDependency = 424,
    TooEarly = 425,
    UpgradeRequired = 426,
    PreconditionRequired = 428,
    TooManyRequests = 429,
    RequestHeaderFieldsTooLarge = 431,
    UnavailableForLegalReasons = 451,
    InternalServerError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    GatewayTimeout = 504,
    HttpVersionNotSupported = 505,
    VariantAlsoNegotiates = 506,
    InsufficientStorage = 507,
    LoopDetected = 508,
    NotExtended = 510,
    NetworkAuthenticationRequired = 511,
}

/// Error returned when converting an unknown code into `HttpStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unsupported standard HTTP status code: {0}")]
pub struct InvalidHttpStatus(pub u16);

impl HttpStatus {
    /// Returns the numeric status code.
    ///
    /// ```rust
    /// use nivasa_common::HttpStatus;
    ///
    /// assert_eq!(HttpStatus::Ok.as_u16(), 200);
    /// ```
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    /// Returns the canonical reason phrase for the status.
    ///
    /// ```rust
    /// use nivasa_common::HttpStatus;
    ///
    /// assert_eq!(HttpStatus::InternalServerError.reason_phrase(), "Internal Server Error");
    /// ```
    pub const fn reason_phrase(self) -> &'static str {
        match self {
            Self::Continue => "Continue",
            Self::SwitchingProtocols => "Switching Protocols",
            Self::Processing => "Processing",
            Self::EarlyHints => "Early Hints",
            Self::Ok => "OK",
            Self::Created => "Created",
            Self::Accepted => "Accepted",
            Self::NonAuthoritativeInformation => "Non-Authoritative Information",
            Self::NoContent => "No Content",
            Self::ResetContent => "Reset Content",
            Self::PartialContent => "Partial Content",
            Self::MultiStatus => "Multi-Status",
            Self::AlreadyReported => "Already Reported",
            Self::ImUsed => "IM Used",
            Self::MultipleChoices => "Multiple Choices",
            Self::MovedPermanently => "Moved Permanently",
            Self::Found => "Found",
            Self::SeeOther => "See Other",
            Self::NotModified => "Not Modified",
            Self::UseProxy => "Use Proxy",
            Self::TemporaryRedirect => "Temporary Redirect",
            Self::PermanentRedirect => "Permanent Redirect",
            Self::BadRequest => "Bad Request",
            Self::Unauthorized => "Unauthorized",
            Self::PaymentRequired => "Payment Required",
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::NotAcceptable => "Not Acceptable",
            Self::ProxyAuthenticationRequired => "Proxy Authentication Required",
            Self::RequestTimeout => "Request Timeout",
            Self::Conflict => "Conflict",
            Self::Gone => "Gone",
            Self::LengthRequired => "Length Required",
            Self::PreconditionFailed => "Precondition Failed",
            Self::PayloadTooLarge => "Payload Too Large",
            Self::UriTooLong => "URI Too Long",
            Self::UnsupportedMediaType => "Unsupported Media Type",
            Self::RangeNotSatisfiable => "Range Not Satisfiable",
            Self::ExpectationFailed => "Expectation Failed",
            Self::ImATeapot => "I'm a teapot",
            Self::MisdirectedRequest => "Misdirected Request",
            Self::UnprocessableEntity => "Unprocessable Entity",
            Self::Locked => "Locked",
            Self::FailedDependency => "Failed Dependency",
            Self::TooEarly => "Too Early",
            Self::UpgradeRequired => "Upgrade Required",
            Self::PreconditionRequired => "Precondition Required",
            Self::TooManyRequests => "Too Many Requests",
            Self::RequestHeaderFieldsTooLarge => "Request Header Fields Too Large",
            Self::UnavailableForLegalReasons => "Unavailable For Legal Reasons",
            Self::InternalServerError => "Internal Server Error",
            Self::NotImplemented => "Not Implemented",
            Self::BadGateway => "Bad Gateway",
            Self::ServiceUnavailable => "Service Unavailable",
            Self::GatewayTimeout => "Gateway Timeout",
            Self::HttpVersionNotSupported => "HTTP Version Not Supported",
            Self::VariantAlsoNegotiates => "Variant Also Negotiates",
            Self::InsufficientStorage => "Insufficient Storage",
            Self::LoopDetected => "Loop Detected",
            Self::NotExtended => "Not Extended",
            Self::NetworkAuthenticationRequired => "Network Authentication Required",
        }
    }

    /// Returns the matching `http::StatusCode`.
    ///
    /// ```rust
    /// use nivasa_common::HttpStatus;
    ///
    /// assert_eq!(HttpStatus::NotFound.to_http_status_code().as_u16(), 404);
    /// ```
    pub fn to_http_status_code(self) -> StatusCode {
        StatusCode::from_u16(self.as_u16())
            .expect("HttpStatus variants are valid standard HTTP status codes")
    }

    /// Returns the matching `HttpException` with the provided message.
    ///
    /// ```rust
    /// use nivasa_common::HttpStatus;
    ///
    /// let ex = HttpStatus::BadRequest.into_exception("invalid payload");
    /// assert_eq!(ex.status_code, 400);
    /// assert_eq!(ex.message, "invalid payload");
    /// ```
    pub fn into_exception(self, message: impl Into<String>) -> HttpException {
        HttpException::from_status(self, message)
    }

    /// Returns `true` if the status is in the 1xx range.
    pub const fn is_informational(self) -> bool {
        matches!(self.as_u16(), 100..=199)
    }

    /// Returns `true` if the status is in the 2xx range.
    pub const fn is_success(self) -> bool {
        matches!(self.as_u16(), 200..=299)
    }

    /// Returns `true` if the status is in the 3xx range.
    pub const fn is_redirection(self) -> bool {
        matches!(self.as_u16(), 300..=399)
    }

    /// Returns `true` if the status is in the 4xx range.
    pub const fn is_client_error(self) -> bool {
        matches!(self.as_u16(), 400..=499)
    }

    /// Returns `true` if the status is in the 5xx range.
    pub const fn is_server_error(self) -> bool {
        matches!(self.as_u16(), 500..=599)
    }
}

impl From<HttpStatus> for u16 {
    fn from(status: HttpStatus) -> Self {
        status.as_u16()
    }
}

impl From<HttpStatus> for StatusCode {
    fn from(status: HttpStatus) -> Self {
        status.to_http_status_code()
    }
}

impl TryFrom<u16> for HttpStatus {
    type Error = InvalidHttpStatus;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        let status = match value {
            100 => Self::Continue,
            101 => Self::SwitchingProtocols,
            102 => Self::Processing,
            103 => Self::EarlyHints,
            200 => Self::Ok,
            201 => Self::Created,
            202 => Self::Accepted,
            203 => Self::NonAuthoritativeInformation,
            204 => Self::NoContent,
            205 => Self::ResetContent,
            206 => Self::PartialContent,
            207 => Self::MultiStatus,
            208 => Self::AlreadyReported,
            226 => Self::ImUsed,
            300 => Self::MultipleChoices,
            301 => Self::MovedPermanently,
            302 => Self::Found,
            303 => Self::SeeOther,
            304 => Self::NotModified,
            305 => Self::UseProxy,
            307 => Self::TemporaryRedirect,
            308 => Self::PermanentRedirect,
            400 => Self::BadRequest,
            401 => Self::Unauthorized,
            402 => Self::PaymentRequired,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            405 => Self::MethodNotAllowed,
            406 => Self::NotAcceptable,
            407 => Self::ProxyAuthenticationRequired,
            408 => Self::RequestTimeout,
            409 => Self::Conflict,
            410 => Self::Gone,
            411 => Self::LengthRequired,
            412 => Self::PreconditionFailed,
            413 => Self::PayloadTooLarge,
            414 => Self::UriTooLong,
            415 => Self::UnsupportedMediaType,
            416 => Self::RangeNotSatisfiable,
            417 => Self::ExpectationFailed,
            418 => Self::ImATeapot,
            421 => Self::MisdirectedRequest,
            422 => Self::UnprocessableEntity,
            423 => Self::Locked,
            424 => Self::FailedDependency,
            425 => Self::TooEarly,
            426 => Self::UpgradeRequired,
            428 => Self::PreconditionRequired,
            429 => Self::TooManyRequests,
            431 => Self::RequestHeaderFieldsTooLarge,
            451 => Self::UnavailableForLegalReasons,
            500 => Self::InternalServerError,
            501 => Self::NotImplemented,
            502 => Self::BadGateway,
            503 => Self::ServiceUnavailable,
            504 => Self::GatewayTimeout,
            505 => Self::HttpVersionNotSupported,
            506 => Self::VariantAlsoNegotiates,
            507 => Self::InsufficientStorage,
            508 => Self::LoopDetected,
            510 => Self::NotExtended,
            511 => Self::NetworkAuthenticationRequired,
            _ => return Err(InvalidHttpStatus(value)),
        };

        Ok(status)
    }
}

impl TryFrom<StatusCode> for HttpStatus {
    type Error = InvalidHttpStatus;

    fn try_from(value: StatusCode) -> Result<Self, Self::Error> {
        Self::try_from(value.as_u16())
    }
}

impl fmt::Display for HttpStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.as_u16(), self.reason_phrase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_to_numeric_and_phrase() {
        assert_eq!(HttpStatus::NotFound.as_u16(), 404);
        assert_eq!(HttpStatus::NotFound.reason_phrase(), "Not Found");
        assert_eq!(HttpStatus::ImATeapot.to_string(), "418 I'm a teapot");
    }

    #[test]
    fn converts_from_numeric_and_http_status_code() {
        assert_eq!(
            HttpStatus::try_from(503).unwrap(),
            HttpStatus::ServiceUnavailable
        );
        assert_eq!(
            HttpStatus::try_from(StatusCode::NOT_FOUND).unwrap(),
            HttpStatus::NotFound
        );
        assert!(HttpStatus::try_from(777).is_err());
    }

    #[test]
    fn converts_into_http_exception() {
        let ex = HttpStatus::UnprocessableEntity.into_exception("Validation failed");
        assert_eq!(ex.status_code, 422);
        assert_eq!(ex.error, "Unprocessable Entity");
        assert_eq!(ex.message, "Validation failed");
    }

    #[test]
    fn class_helpers_work() {
        assert!(HttpStatus::Ok.is_success());
        assert!(HttpStatus::Created.is_success());
        assert!(HttpStatus::Found.is_redirection());
        assert!(HttpStatus::BadRequest.is_client_error());
        assert!(HttpStatus::InternalServerError.is_server_error());
        assert!(HttpStatus::Continue.is_informational());
    }
}
