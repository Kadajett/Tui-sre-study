use std::{env, io::Read, time::Duration};

use anyhow::{ensure, Context, Result};
use reqwest::{blocking::Client, redirect::Policy};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct Reference {
    pub text: String,
    pub sources: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    query: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocQuery {
    doc: String,
    query: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    doc: String,
    path: String,
}
#[derive(Deserialize)]
struct CatalogDoc {
    name: String,
    slug: String,
}
#[derive(Deserialize)]
struct Index {
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    name: String,
    path: String,
}
#[derive(Deserialize)]
struct Search {
    results: Vec<SearchResult>,
}
#[derive(Deserialize)]
struct SearchResult {
    title: String,
    url: String,
    #[serde(default)]
    content: String,
}

pub fn definitions() -> Value {
    json!([
        {"type":"function","function":{"name":"list_docs","description":"Find document collections in the user's DevDocs catalog. Use an empty query to list SRE-relevant collections, or a topic/name. Catalog entries are not proof that every page is installed; search_docs verifies its index.","parameters":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"search_docs","description":"Search entry titles in a DevDocs collection. Use a slug returned by list_docs, such as bash or kubernetes. Returns exact paths for read_doc.","parameters":{"type":"object","properties":{"doc":{"type":"string"},"query":{"type":"string"}},"required":["doc","query"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"read_doc","description":"Read a documentation page from the user's DevDocs. Use the doc slug and path returned by search_docs.","parameters":{"type":"object","properties":{"doc":{"type":"string"},"path":{"type":"string"}},"required":["doc","path"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"search_web","description":"Search through the user's private SearXNG service. Prefer official primary sources using site: filters. Never search secrets or private infrastructure data. Results contain snippets, not full verified pages.","parameters":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}}}
    ])
}

pub fn lookup(name: &str, arguments: &str) -> Result<Reference> {
    ensure!(arguments.len() <= 2000, "Lookup arguments are too long");
    let client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(35))
        .build()?;
    match name {
        "list_docs" => catalog(&client, serde_json::from_str(arguments)?),
        "search_docs" => search_docs(&client, serde_json::from_str(arguments)?),
        "read_doc" => read_doc(&client, serde_json::from_str(arguments)?),
        "search_web" => search_web(&client, serde_json::from_str(arguments)?),
        _ => anyhow::bail!("Unknown reference tool"),
    }
}

fn base(variable: &str, fallback: &str) -> Result<String> {
    let base = env::var(variable).unwrap_or_else(|_| fallback.to_owned());
    let url = reqwest::Url::parse(&base)?;
    ensure!(
        url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| host.ends_with(".tailf93a13.ts.net")),
        "Reference services must use your Tailscale HTTPS domain"
    );
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Reference URL cannot contain credentials, a query or fragment"
    );
    Ok(base.trim_end_matches('/').to_owned())
}

fn docs_base() -> Result<String> {
    base("DEVDOCS_URL", "https://devdocs.tailf93a13.ts.net")
}

fn fetch(client: &Client, url: &str) -> Result<String> {
    let response = client.get(url).send()?.error_for_status()?;
    let mut bytes = Vec::new();
    response.take(2_000_001).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 2_000_000,
        "Reference page is too large; use a narrower collection"
    );
    Ok(String::from_utf8(bytes)?)
}

fn catalog(client: &Client, query: Query) -> Result<Reference> {
    let url = format!("{}/assets/docs.js", docs_base()?);
    let body = fetch(client, &url)?;
    let start = body.find('[').context("DevDocs did not return a catalog")?;
    let mut decoder = serde_json::Deserializer::from_str(&body[start..]);
    let docs = Vec::<CatalogDoc>::deserialize(&mut decoder)?;
    let words: Vec<String> = if query.query.trim().is_empty() {
        [
            "bash",
            "docker",
            "kubernetes",
            "nginx",
            "postgres",
            "redis",
            "ansible",
            "terraform",
            "http",
            "prometheus",
            "git",
        ]
        .iter()
        .map(|word| (*word).to_owned())
        .collect()
    } else {
        query
            .query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    };
    let entries: Vec<Value> = docs
        .iter()
        .filter(|doc| {
            words.iter().any(|word| {
                format!("{} {}", doc.name, doc.slug)
                    .to_lowercase()
                    .contains(word)
            })
        })
        .take(40)
        .map(|doc| json!({"name":doc.name,"slug":doc.slug}))
        .collect();
    Ok(Reference { text: json!({"catalog_size":docs.len(),"matching_collections":entries,"note":"Verify index availability with search_docs; this list includes catalog entries."}).to_string(), sources: vec![url] })
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 80
        && slug
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.~".contains(&b))
        && !slug.contains("..")
}

fn search_docs(client: &Client, query: DocQuery) -> Result<Reference> {
    ensure!(valid_slug(&query.doc), "Invalid documentation slug");
    let base = docs_base()?;
    let index_url = format!("{base}/docs/{}/index.json", query.doc);
    let index: Index = serde_json::from_str(&fetch(client, &index_url)?)
        .context("This DevDocs collection has no usable installed index")?;
    let words: Vec<String> = query
        .query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let matches: Vec<Value> = index.entries.iter().filter(|entry| words.iter().all(|word| format!("{} {}", entry.name, entry.path).to_lowercase().contains(word)))
        .take(8).map(|entry| json!({"name":entry.name,"path":entry.path,"url":format!("{base}/{}/{}",query.doc,entry.path)})).collect();
    Ok(Reference { text: json!({"matches":matches,"note":"Title matches only. Use read_doc for the page content. Empty matches may need a shorter query."}).to_string(), sources: vec![index_url] })
}

fn read_doc(client: &Client, page: Page) -> Result<Reference> {
    ensure!(valid_slug(&page.doc), "Invalid documentation slug");
    let path = page.path.split('#').next().unwrap_or("");
    ensure!(
        !path.is_empty()
            && path.len() <= 300
            && !path.starts_with('/')
            && !path.contains("..")
            && path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.~/".contains(&b)),
        "Use the relative path returned by search_docs"
    );
    let base = docs_base()?;
    let url = format!("{base}/docs/{}/{path}.html", page.doc);
    let body = fetch(client, &url)?;
    ensure!(
        !body.contains("class=\"_booting\""),
        "DevDocs returned its application shell instead of this documentation page"
    );
    Ok(Reference {
        text: html_text(&body).chars().take(12_000).collect(),
        sources: vec![format!("{base}/{}/{}", page.doc, page.path)],
    })
}

fn search_web(client: &Client, query: Query) -> Result<Reference> {
    ensure!(
        !query.query.trim().is_empty() && query.query.len() <= 500,
        "Search query must be 1–500 characters"
    );
    let base = base("SEARXNG_URL", "https://searxng.tailf93a13.ts.net")?;
    let mut url = reqwest::Url::parse(&format!("{base}/search"))?;
    url.query_pairs_mut()
        .append_pair("q", &query.query)
        .append_pair("format", "json");
    let body = fetch(client, url.as_str())?;
    let results: Search =
        serde_json::from_str(&body).context("SearXNG did not return JSON search results")?;
    let selected: Vec<_> = results.results.into_iter().take(4).collect();
    let sources = selected.iter().map(|item| item.url.clone()).collect();
    let text = selected
        .into_iter()
        .map(|item| {
            format!(
                "{}\n{}\n{}",
                item.title,
                item.url,
                item.content.chars().take(1000).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok(Reference {
        text: if text.is_empty() {
            "No search results".into()
        } else {
            text
        },
        sources,
    })
}

fn html_text(html: &str) -> String {
    let mut output = String::new();
    let mut inside_tag = false;
    for character in html.chars() {
        match character {
            '<' => {
                inside_tag = true;
                output.push(' ');
            }
            '>' => inside_tag = false,
            _ if !inside_tag => output.push(character),
            _ => {}
        }
    }
    output
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_slugs_cannot_change_the_origin_or_escape_the_collection() {
        assert!(valid_slug("postgresql~16"));
        for invalid in [
            "../bash",
            "https://example.com",
            "bash/../../",
            "bash?x=1",
            "bash%2f..",
        ] {
            assert!(!valid_slug(invalid));
        }
    }
    #[test]
    fn markup_is_converted_to_plain_reference_text() {
        assert_eq!(
            html_text("<h1>Exit status</h1><p>0 means success &amp; nonzero means failure.</p>"),
            "Exit status 0 means success & nonzero means failure."
        );
    }
}
