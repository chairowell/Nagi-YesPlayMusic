//! Cookie hardening shared by the Sidecar's HTTP boundary and any future
//! consumer that relays NCM Set-Cookie headers.

fn cookie_name(cookie: &str) -> &str {
    cookie
        .split_once('=')
        .map(|(name, _)| name.trim())
        .unwrap_or_default()
}

pub fn harden_auth_cookie(cookie: &str) -> String {
    if !matches!(cookie_name(cookie), "MUSIC_U" | "__csrf") {
        return cookie.to_owned();
    }
    let mut parts = cookie
        .split(';')
        .map(str::trim)
        .filter(|part| {
            !part.is_empty()
                && !part.eq_ignore_ascii_case("httponly")
                && !part.to_ascii_lowercase().starts_with("samesite=")
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    parts.push("HttpOnly".to_owned());
    parts.push("SameSite=Strict".to_owned());
    parts.join("; ")
}

pub fn desktop_session_expiry_cookies() -> [String; 2] {
    let attributes =
        "Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; HttpOnly; SameSite=Strict";
    [
        format!("MUSIC_U=; {attributes}"),
        format!("__csrf=; {attributes}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_cookies_are_hardened_without_collapsing_other_cookies() {
        assert_eq!(
            harden_auth_cookie("MUSIC_U=value; Path=/; SameSite=Lax"),
            "MUSIC_U=value; Path=/; HttpOnly; SameSite=Strict"
        );
        assert_eq!(
            harden_auth_cookie("NMTID=value; Path=/"),
            "NMTID=value; Path=/"
        );
        assert_eq!(desktop_session_expiry_cookies().len(), 2);
    }
}
