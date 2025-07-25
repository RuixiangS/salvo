use std::convert::Infallible;
use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::io::Error as IoError;

use crate::http::{ParseError, StatusError};
use crate::{Response, Scribe};

/// `BoxedError` is a boxed error type that can be used as a trait object.
pub type BoxedError = Box<dyn StdError + Send + Sync>;

/// Errors that can happen inside salvo.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error occurred in hyper.
    Hyper(hyper::Error),
    /// Error happened when parse http.
    HttpParse(ParseError),
    /// Error from http response error status.
    HttpStatus(StatusError),
    /// Std I/O error.
    Io(IoError),
    /// SerdeJson error.
    SerdeJson(serde_json::Error),
    /// Invalid URI error.
    InvalidUri(http::uri::InvalidUri),
    #[cfg(feature = "quinn")]
    #[cfg_attr(docsrs, doc(cfg(feature = "quinn")))]
    /// H3 ConnectionError.
    H3Connection(salvo_http3::error::ConnectionError),
    #[cfg(feature = "quinn")]
    #[cfg_attr(docsrs, doc(cfg(feature = "quinn")))]
    /// H3 StreamError.
    H3Stream(salvo_http3::error::StreamError),
    #[cfg(feature = "quinn")]
    #[cfg_attr(docsrs, doc(cfg(feature = "quinn")))]
    /// H3 SendDatagramError.
    H3SendDatagram(h3_datagram::datagram_handler::SendDatagramError),
    /// Anyhow error.
    #[cfg(feature = "anyhow")]
    #[cfg_attr(docsrs, doc(cfg(feature = "anyhow")))]
    Anyhow(anyhow::Error),
    /// Anyhow error.
    #[cfg(feature = "eyre")]
    #[cfg_attr(docsrs, doc(cfg(feature = "eyre")))]
    Eyre(eyre::Report),
    /// Custom error that does not fall under any other error kind.
    Other(BoxedError),
}

impl Error {
    /// Create a custom error.
    #[inline]
    pub fn other(error: impl Into<BoxedError>) -> Self {
        Self::Other(error.into())
    }
}
impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Hyper(e) => Display::fmt(e, f),
            Self::HttpParse(e) => Display::fmt(e, f),
            Self::HttpStatus(e) => Display::fmt(e, f),
            Self::Io(e) => Display::fmt(e, f),
            Self::SerdeJson(e) => Display::fmt(e, f),
            Self::InvalidUri(e) => Display::fmt(e, f),
            #[cfg(feature = "quinn")]
            Self::H3Connection(e) => Display::fmt(e, f),
            #[cfg(feature = "quinn")]
            Self::H3Stream(e) => Display::fmt(e, f),
            #[cfg(feature = "quinn")]
            Self::H3SendDatagram(e) => Display::fmt(e, f),
            #[cfg(feature = "anyhow")]
            Self::Anyhow(e) => Display::fmt(e, f),
            #[cfg(feature = "eyre")]
            Self::Eyre(e) => Display::fmt(e, f),
            Self::Other(e) => Display::fmt(e, f),
        }
    }
}

impl StdError for Error {}

impl From<Infallible> for Error {
    #[inline]
    fn from(infallible: Infallible) -> Self {
        match infallible {}
    }
}
impl From<hyper::Error> for Error {
    #[inline]
    fn from(e: hyper::Error) -> Self {
        Self::Hyper(e)
    }
}
impl From<ParseError> for Error {
    #[inline]
    fn from(d: ParseError) -> Self {
        Self::HttpParse(d)
    }
}
impl From<StatusError> for Error {
    #[inline]
    fn from(e: StatusError) -> Self {
        Self::HttpStatus(e)
    }
}
impl From<IoError> for Error {
    #[inline]
    fn from(e: IoError) -> Self {
        Self::Io(e)
    }
}
impl From<http::uri::InvalidUri> for Error {
    #[inline]
    fn from(e: http::uri::InvalidUri) -> Self {
        Self::InvalidUri(e)
    }
}
impl From<serde_json::Error> for Error {
    #[inline]
    fn from(e: serde_json::Error) -> Self {
        Self::SerdeJson(e)
    }
}
cfg_feature! {
    #![feature = "quinn"]
    impl From<salvo_http3::error::ConnectionError> for Error {
        #[inline]
        fn from(e: salvo_http3::error::ConnectionError) -> Error {
            Error::H3Connection(e)
        }
    }
    impl From<salvo_http3::error::StreamError> for Error {
        #[inline]
        fn from(e: salvo_http3::error::StreamError) -> Error {
            Error::H3Stream(e)
        }
    }
    impl From<h3_datagram::datagram_handler::SendDatagramError> for Error {
        #[inline]
        fn from(e: h3_datagram::datagram_handler::SendDatagramError) -> Error {
            Error::H3SendDatagram(e)
        }
    }
}
cfg_feature! {
    #![feature = "anyhow"]
    impl From<anyhow::Error> for Error {
        #[inline]
        fn from(e: anyhow::Error) -> Self {
            Self::Anyhow(e)
        }
    }
}
cfg_feature! {
    #![feature = "eyre"]
    impl From<eyre::Report> for Error {
        #[inline]
        fn from(e: eyre::Report) -> Error {
            Error::Eyre(e)
        }
    }
}

impl From<BoxedError> for Error {
    #[inline]
    fn from(e: BoxedError) -> Self {
        Self::Other(e)
    }
}

impl Scribe for Error {
    fn render(self, res: &mut Response) {
        let status_error = match self {
            Self::HttpStatus(e) => e,
            _ => StatusError::internal_server_error().cause(self),
        };
        res.render(status_error);
    }
}
cfg_feature! {
    #![feature = "anyhow"]
    impl Scribe for anyhow::Error {
        #[inline]
        fn render(self, res: &mut Response) {
            tracing::error!(error = ?self, "anyhow error occurred");
            res.render(StatusError::internal_server_error().origin(self));
        }
    }
}
cfg_feature! {
    #![feature = "eyre"]
    impl Scribe for eyre::Report {
        #[inline]
        fn render(self, res: &mut Response) {
            tracing::error!(error = ?self, "eyre error occurred");
            res.render(StatusError::internal_server_error().cause(self));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::http::*;
    use crate::{Depot, Writer};

    use super::*;

    #[tokio::test]
    #[cfg(feature = "anyhow")]
    async fn test_anyhow() {
        let mut req = Request::default();
        let mut res = Response::default();
        let mut depot = Depot::new();
        let e: anyhow::Error = anyhow::anyhow!("detail message");
        e.write(&mut req, &mut depot, &mut res).await;
        assert_eq!(res.status_code, Some(StatusCode::INTERNAL_SERVER_ERROR));
    }

    #[tokio::test]
    #[cfg(feature = "eyre")]
    async fn test_eyre() {
        let mut req = Request::default();
        let mut res = Response::default();
        let mut depot = Depot::new();
        let e: eyre::Report = eyre::Report::msg("detail message");
        e.write(&mut req, &mut depot, &mut res).await;
        assert_eq!(res.status_code, Some(StatusCode::INTERNAL_SERVER_ERROR));
    }

    #[tokio::test]
    async fn test_error() {
        let mut req = Request::default();
        let mut res = Response::default();
        let mut depot = Depot::new();

        let e = Error::Other("detail message".into());
        e.write(&mut req, &mut depot, &mut res).await;
        assert_eq!(res.status_code, Some(StatusCode::INTERNAL_SERVER_ERROR));
    }
}
