use actix_files::{Files, NamedFile};
use actix_identity::{CookieIdentityPolicy, Identity, IdentityService};
use actix_web::{http::header, web, App, HttpRequest, HttpResponse, HttpServer};
use anyhow::Result;
use tokio::time::{delay_for, Duration};

async fn index() -> actix_web::Result<NamedFile> {
    Ok(NamedFile::open("./client/index.html")?)
}

/// Handle a login request by getting a basic auth header from the
/// incoming request and verifying those credentials.
async fn login(id: Identity, req: HttpRequest) -> HttpResponse {
    let token = 
        req
            .headers()
            .get(header::AUTHORIZATION)
            .unwrap()
            .to_str()
            .unwrap()
            .split(' ')
            .last()
            .unwrap();

    let decoded_token = String::from_utf8(base64::decode(token).unwrap()).unwrap();
    let mut token_data = decoded_token.split(":");
    let user = token_data.next().unwrap().to_owned();

    /* Check basic auth credentials are ok.
    this process may take a few seconds...
    HINT: before I added this delay, my app appeared
    to work as I wanted/expected -- a response came
    back from the server before the browser was able
    to refresh itself.  With this delay, things break.
    Some wireshark sniffing shows that the reason is
    that, during this delay, the browser refreshes the
    seed app and its listen port changes -- a request
    is made to /auth/login from port 33086, for example,
    but by the time the server responds, the seed app
    is listening on port 33090!!! */
    delay_for(Duration::from_secs(3)).await;

    /* add a secure cookie to the http response */
    println!("Logging in user: {}", user);
    id.remember(user.clone());

    HttpResponse::Ok().body(user)
}

async fn logout(id: Identity) -> HttpResponse {
    id.forget();
    println!("User logged out");
    HttpResponse::Ok().finish()
}

/// This function checks whether or not there is a logged-in
/// user by looking at the identity cookie.  If there is a
/// user, it returns a username for the seed/wasm app to
/// use; if not, it returns an empty response.
async fn check_login(id: Identity) -> HttpResponse {
    if let Some(user) = id.identity() {
        HttpResponse::Ok().body(user)
    } else {
        HttpResponse::Ok().finish()
    }
}

#[actix_rt::main]
async fn main() -> Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(b"abcdefghijklmnopqrstuvwxyz123456")
                    .name("special-cookie")
                    .secure(false),
            ))
            .service(
                web::scope("/auth")
                    .route("/check", web::get().to(check_login))
                    .route("/login", web::get().to(login))
                    .route("/logout", web::get().to(logout))
            )
            .service(Files::new("/pkg", "./client/pkg"))
            .default_service(web::get().to(index))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await?;

    Ok(())
}
