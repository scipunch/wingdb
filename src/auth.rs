use axum::{
    async_trait,
    response::IntoResponse,
    routing::{get, post},
    Form, Router,
};
use axum_login::{AuthUser, AuthnBackend, UserId};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    id: i64,
    pub username: String,
    password: String,
}

impl User {
    pub fn from_env() -> Self {
        Self {
            id: 1,
            username: "admin".to_string(),
            password: "password".to_string(),
        }
    }
}

impl AuthUser for User {
    type Id = i64;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.password.as_bytes() // We use the password hash as the auth
                                 // hash--what this means
                                 // is when the user changes their password the
                                 // auth session becomes invalid.
    }
}

#[derive(Debug, Clone)]
pub struct Backend {
    user: User,
}

impl Backend {
    pub fn new(user: User) -> Self {
        Self { user }
    }
}

#[derive(Debug)]
pub struct AuthError(anyhow::Error);

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
impl std::error::Error for AuthError {}

#[async_trait]
impl AuthnBackend for Backend {
    type User = User;
    type Credentials = Credentials;
    type Error = AuthError;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        if creds.username == self.user.username && creds.password == self.user.password {
            Ok(Some(self.user.clone()))
        } else {
            Ok(None)
        }
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        if *user_id == self.user.id {
            Ok(Some(self.user.clone()))
        } else {
            Ok(None)
        }
    }
}

// We use a type alias for convenience.
//
// Note that we've supplied our concrete backend here.
pub type AuthSession = axum_login::AuthSession<Backend>;

// This allows us to extract the authentication fields from forms. We use this
// to authenticate requests with the backend.
#[derive(Debug, Clone, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub next: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(post::login))
        .route("/login", get(get::login))
        .route("/logout", get(get::logout))
}
mod post {
    use axum::{http::StatusCode, response::Redirect};

    use super::*;

    pub async fn login(
        mut auth_session: AuthSession,
        Form(creds): Form<Credentials>,
    ) -> impl IntoResponse {
        let user = match auth_session.authenticate(creds.clone()).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                let mut login_url = "/login".to_string();
                if let Some(next) = creds.next {
                    login_url = format!("{}?next={}", login_url, next);
                };

                return Redirect::to(&login_url).into_response();
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };

        if auth_session.login(&user).await.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }

        if let Some(ref next) = creds.next {
            Redirect::to(next)
        } else {
            Redirect::to("/")
        }
        .into_response()
    }
}

mod get {
    use axum::{
        http::StatusCode,
        response::{Html, Redirect},
    };

    use super::*;

    pub async fn login() -> impl IntoResponse {
        Html(std::include_str!("web/pages/login.html"))
    }

    pub async fn logout(mut auth_session: AuthSession) -> impl IntoResponse {
        match auth_session.logout().await {
            Ok(_) => Redirect::to("/login").into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}