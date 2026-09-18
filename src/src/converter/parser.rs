use super::ss::parse_ss;
use super::trojan::parse_trojan;
use super::types::Proxy;
use super::vless::parse_vless;
use super::vmess::parse_vmess;

pub fn parse_link(link: &str) -> Result<Proxy, String> {
    let link = link.trim();

    if link.starts_with("vmess://") {
        parse_vmess(link)
    } else if link.starts_with("vless://") {
        parse_vless(link)
    } else if link.starts_with("trojan://") {
        parse_trojan(link)
    } else if link.starts_with("ss://") {
        parse_ss(link)
    } else {
        let prefix_len = std::cmp::min(20, link.len());
        Err(format!("unsupported link format: {}", &link[..prefix_len]))
    }
}

pub fn parse_links(links: &[String]) -> Result<Vec<Proxy>, String> {
    let mut proxies = Vec::new();
    let mut errors = Vec::new();

    for link in links {
        let trimmed = link.trim();
        if trimmed.is_empty() {
            continue;
        }

        match parse_link(trimmed) {
            Ok(p) => proxies.push(p),
            Err(e) => errors.push(e),
        }
    }

    if proxies.is_empty() && !errors.is_empty() {
        return Err(format!("failed to parse any links: {:?}", errors));
    }

    Ok(proxies)
}
