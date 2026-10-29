//! What can go wrong talking to easytier-web, in the terms the console UI acts on.
//!
//! Messages never carry passwords, session cookies, network secrets or request bodies: a server's own `message`
//! text is passed on (it describes the refusal, not the request) and a decode failure reports only where in the
//! reply the shape was wrong, because serde's own text can echo the offending value.

/// Why a request to easytier-web did not give the answer the caller wanted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RemoteError {
    /// Not signed in, or the session is gone (a restart of easytier-web, 24 h of age, a password change): the
    /// server answered 401 with an empty body. Sign in again.
    #[error("not signed in to the server, or the session ended")]
    Unauthorized,
    /// The login was refused: wrong username or password (401 with a message body).
    #[error("the server refused the username or password")]
    BadCredentials,
    /// The machine (or its session) is not connected to the server any more.
    #[error("the server does not know this machine: {0}")]
    NotFound(String),
    /// The server understood and refused, with its own words (`{"message": ".."}`), or answered something that is
    /// not a success and not one of the cases above.
    #[error("the server answered {status}: {message}")]
    Rejected {
        /// The HTTP status.
        status: u16,
        /// The server's message, or `HTTP <status>` when it sent none.
        message: String,
    },
    /// The server did not answer in time.
    #[error("the server did not answer in time")]
    Timeout,
    /// easytier-web answered, but the machine behind it did not: the server's call to the machine's core failed
    /// or timed out (502 / 504 with easytier-web's own `{"message"}` body). The server itself is fine.
    #[error("the machine did not answer the server: {0}")]
    MachineNoAnswer(String),
    /// The request never got an answer: the network, DNS, TLS or (in the browser) CORS failed.
    #[error("could not reach the server: {0}")]
    Network(String),
    /// The reply was not the shape this client reads; the text says where, never what.
    #[error("the server's reply could not be read: {0}")]
    Decode(String),
    /// A remote server was addressed over plain HTTP; only HTTPS and loopback are allowed because the login
    /// sends an unsalted MD5 of the password.
    #[error("plain HTTP is only allowed to this machine; use https://")]
    InsecureTransport,
    /// The server address is not a usable base URL.
    #[error("the server address is not valid: {0}")]
    InvalidAddress(String),
    /// The request was not sent: it is inconsistent (for example a config for another instance).
    #[error("the request is not valid: {0}")]
    Invalid(String),
}

impl RemoteError {
    /// Whether signing in again could fix it.
    pub fn needs_login(&self) -> bool {
        matches!(self, Self::Unauthorized)
    }
}
