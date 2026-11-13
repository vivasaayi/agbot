use reqwest::{
    blocking::{Client, RequestBuilder, Response},
    Method, Url,
};
use std::sync::OnceLock;

fn is_local_api(base: &str, target: &str) -> bool {
    let (Ok(base), Ok(target)) = (Url::parse(base), Url::parse(target)) else {
        return false;
    };
    let secure = base.scheme() == "https"
        || (base.scheme() == "http"
            && base.host_str().is_some_and(|host| {
                host.eq_ignore_ascii_case("localhost")
                    || host
                        .trim_matches(['[', ']'])
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
            }));
    secure
        && base.username().is_empty()
        && base.password().is_none()
        && base.scheme() == target.scheme()
        && base.host() == target.host()
        && base.port_or_known_default() == target.port_or_known_default()
        && target.username().is_empty()
        && target.password().is_none()
        && target.path().starts_with("/api/")
}

fn make_request(method: Method, target: &str, base: &str, token: Option<&str>) -> RequestBuilder {
    static PLAIN: OnceLock<Client> = OnceLock::new();
    static AUTHENTICATED: OnceLock<Client> = OnceLock::new();
    let token = token.filter(|token| !token.is_empty() && is_local_api(base, target));
    let client = if token.is_some() {
        AUTHENTICATED.get_or_init(|| {
            Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("HTTP client")
        })
    } else {
        PLAIN.get_or_init(Client::new)
    };
    let request = client.request(method, target);
    match token {
        Some(token) => request.bearer_auth(token),
        None => request,
    }
}

pub fn request(method: Method, target: &str) -> RequestBuilder {
    let base = std::env::var("GEO_HUB_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let token = std::env::var("GEO_VIEWER_SESSION_TOKEN").ok();
    make_request(method, target, &base, token.as_deref())
}

pub fn get(target: &str) -> reqwest::Result<Response> {
    request(Method::GET, target).send()
}

pub fn check_connection(base: &str, token: &str) -> anyhow::Result<()> {
    use std::io::Read;

    #[derive(serde::Deserialize)]
    struct PortalIdentity {
        account_id: String,
        org_id: String,
        party_type: String,
    }

    let target = format!("{}/api/portal/me", base.trim_end_matches('/'));
    if token.is_empty() || !is_local_api(base, &target) {
        anyhow::bail!("a valid session and secure Geo Hub origin are required");
    }
    let response = make_request(Method::GET, &target, base, Some(token))
        .timeout(std::time::Duration::from_secs(10))
        .send()?;
    if !response.status().is_success() {
        anyhow::bail!("Geo Hub connection check returned {}", response.status());
    }
    let mut body = Vec::new();
    response.take(65_537).read_to_end(&mut body)?;
    if body.len() > 65_536 {
        anyhow::bail!("Geo Hub identity response is too large");
    }
    let identity: PortalIdentity = serde_json::from_slice(&body)
        .map_err(|_| anyhow::anyhow!("Geo Hub returned an invalid identity response"))?;
    if [identity.account_id, identity.org_id, identity.party_type]
        .iter()
        .any(|value| value.trim().is_empty())
    {
        anyhow::bail!("Geo Hub returned an incomplete identity response");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check_connection, is_local_api, make_request};
    use reqwest::{header::AUTHORIZATION, Method};

    #[test]
    fn sessions_are_scoped_to_the_configured_api_origin() {
        let base = "https://viewer.example:8443";
        assert!(is_local_api(
            base,
            "https://viewer.example:8443/api/scenes/a"
        ));
        assert!(!is_local_api(
            base,
            "https://other.example:8443/api/scenes/a"
        ));
        assert!(!is_local_api(
            base,
            "http://viewer.example:8443/api/scenes/a"
        ));
        assert!(!is_local_api(base, "https://viewer.example/api/scenes/a"));
        assert!(!is_local_api(
            base,
            "https://viewer.example:8443/workspace/"
        ));
        assert!(!is_local_api(base, "not a URL"));
        assert!(is_local_api(
            "http://127.0.0.1:20080",
            "http://127.0.0.1:20080/api/farms"
        ));
        assert!(is_local_api(
            "http://[::1]:20080",
            "http://[::1]:20080/api/farms"
        ));
        assert!(!is_local_api(
            "http://192.0.2.1",
            "http://192.0.2.1/api/farms"
        ));
        assert!(!is_local_api(
            "https://user:secret@viewer.example",
            "https://viewer.example/api/farms"
        ));
    }

    #[test]
    fn request_only_attaches_tokens_to_the_matching_api() {
        let base = "https://viewer.example";
        let local = make_request(
            Method::POST,
            "https://viewer.example/api/fields",
            base,
            Some("secret"),
        )
        .build()
        .unwrap();
        assert_eq!(local.headers()[AUTHORIZATION], "Bearer secret");
        let other = make_request(
            Method::GET,
            "https://other.example/api/fields",
            base,
            Some("secret"),
        )
        .build()
        .unwrap();
        assert!(!other.headers().contains_key(AUTHORIZATION));
        let public = make_request(
            Method::GET,
            "https://viewer.example/health",
            base,
            Some("secret"),
        )
        .build()
        .unwrap();
        assert!(!public.headers().contains_key(AUTHORIZATION));
    }

    #[test]
    fn authenticated_requests_do_not_follow_redirects() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut bytes = [0; 4096];
            let n = socket.read(&mut bytes).unwrap();
            assert!(String::from_utf8_lossy(&bytes[..n])
                .to_ascii_lowercase()
                .contains("authorization: bearer secret"));
            socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: https://other.example/api/farms\r\nContent-Length: 0\r\n\r\n").unwrap();
        });
        let response = make_request(
            Method::GET,
            &format!("{base}/api/farms"),
            &base,
            Some("secret"),
        )
        .send()
        .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::FOUND);
        server.join().unwrap();
    }

    #[test]
    fn connection_check_authenticates_and_rejects_errors() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        for (status, body, accepted) in [
            (
                "200 OK",
                r#"{"account_id":"a","org_id":"o","party_type":"farmer"}"#,
                true,
            ),
            ("200 OK", "<html>wrong service</html>", false),
            ("200 OK", r#"{"account_id":"a"}"#, false),
            (
                "200 OK",
                r#"{"account_id":"","org_id":"o","party_type":"farmer"}"#,
                false,
            ),
            ("302 Found", "", false),
            ("401 Unauthorized", "", false),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                let mut bytes = [0; 4096];
                let n = socket.read(&mut bytes).unwrap();
                let request = String::from_utf8_lossy(&bytes[..n]).to_ascii_lowercase();
                assert!(request.starts_with("get /api/portal/me "));
                assert!(request.contains("authorization: bearer secret"));
                write!(
                    socket,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            });
            assert_eq!(check_connection(&base, "secret").is_ok(), accepted);
            server.join().unwrap();
        }
        assert!(check_connection("http://192.0.2.1", "secret").is_err());
        assert!(check_connection("http://127.0.0.1", "").is_err());
    }
}
