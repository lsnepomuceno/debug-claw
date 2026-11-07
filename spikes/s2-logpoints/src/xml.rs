//! A minimal XML reader for the subset Xdebug sends over DBGp: an optional
//! prolog, elements with double-quoted attributes, CDATA sections and plain
//! text. Enough for the spike, not a general parser.

#[derive(Debug, Default)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub text: String,
    pub children: Vec<Node>,
}

impl Node {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
}

pub fn parse(input: &str) -> Result<Node, String> {
    let mut stack: Vec<Node> = Vec::new();
    let mut rest = input;
    let mut root: Option<Node> = None;

    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<![CDATA[") {
            let end = after.find("]]>").ok_or("unterminated CDATA")?;
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&after[..end]);
            }
            rest = &after[end + 3..];
        } else if rest.starts_with("<?") || rest.starts_with("<!") {
            let end = rest.find('>').ok_or("unterminated declaration")?;
            rest = &rest[end + 1..];
        } else if let Some(after) = rest.strip_prefix("</") {
            let end = after.find('>').ok_or("unterminated end tag")?;
            let node = stack.pop().ok_or("unbalanced end tag")?;
            match stack.last_mut() {
                Some(parent) => parent.children.push(node),
                None => root = Some(node),
            }
            rest = &after[end + 1..];
        } else if let Some(after) = rest.strip_prefix('<') {
            let end = find_tag_end(after).ok_or("unterminated start tag")?;
            let body = &after[..end];
            let (body, self_closing) = match body.strip_suffix('/') {
                Some(b) => (b, true),
                None => (body, false),
            };
            let node = parse_start(body)?;
            if self_closing {
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            } else {
                stack.push(node);
            }
            rest = &after[end + 1..];
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            if let Some(top) = stack.last_mut() {
                let text = rest[..end].trim();
                if !text.is_empty() {
                    top.text.push_str(&unescape(text));
                }
            }
            rest = &rest[end..];
        }
    }
    root.ok_or_else(|| "no root element".to_string())
}

/// Finds the closing `>` of a start tag, skipping `>` inside quoted values.
fn find_tag_end(s: &str) -> Option<usize> {
    let mut quoted = false;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'"' => quoted = !quoted,
            b'>' if !quoted => return Some(i),
            _ => {}
        }
    }
    None
}

fn parse_start(body: &str) -> Result<Node, String> {
    let body = body.trim();
    let name_end = body
        .find(|c: char| c.is_ascii_whitespace())
        .unwrap_or(body.len());
    let mut node = Node {
        name: body[..name_end].to_string(),
        ..Node::default()
    };
    let mut rest = body[name_end..].trim_start();
    while !rest.is_empty() {
        let eq = rest.find('=').ok_or("attribute without value")?;
        let key = rest[..eq].trim().to_string();
        let after = rest[eq + 1..].trim_start();
        let after = after.strip_prefix('"').ok_or("unquoted attribute")?;
        let close = after.find('"').ok_or("unterminated attribute")?;
        node.attrs.push((key, unescape(&after[..close])));
        rest = after[close + 1..].trim_start();
    }
    Ok(node)
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Decodes standard base64, ignoring whitespace. Returns None on bad input.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_property_response() {
        let xml = r#"<?xml version="1.0" encoding="iso-8859-1"?>
<response xmlns="urn:debugger_protocol_v1" command="property_get" transaction_id="7"><property name="$item" fullname="$item" type="array" children="1" numchildren="2" page="0" pagesize="32"><property name="index" fullname="$item[&quot;index&quot;]" type="int"><![CDATA[3]]></property><property name="square" fullname="$item[&quot;square&quot;]" type="int"><![CDATA[9]]></property></property></response>"#;
        let root = parse(xml).unwrap();
        assert_eq!(root.name, "response");
        assert_eq!(root.attr("transaction_id"), Some("7"));
        let prop = root.child("property").unwrap();
        assert_eq!(prop.children.len(), 2);
        assert_eq!(prop.children[0].attr("fullname"), Some("$item[\"index\"]"));
        assert_eq!(prop.children[1].text, "9");
    }

    #[test]
    fn decodes_base64() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
    }
}
