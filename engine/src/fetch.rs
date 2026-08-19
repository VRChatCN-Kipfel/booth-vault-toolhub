//! BOOTH 商品元数据：fetch_item JSON / refine_from_json / 价格解析 / 缩略图。

use reqwest::blocking::Client;
use serde::Deserialize;

use crate::http::get;

/// BOOTH 商品 JSON 端点。
pub const ITEM_JSON_URL: &str = "https://booth.pm/ja/items/{id}.json";

/// 商品 JSON 结构（容错字段变动，`#[serde(default)]`）。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ItemJson {
    /// 商品 ID（JSON API 为数字，搜索卡片为字符串，兼容两种）。
    #[serde(deserialize_with = "de_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub price: serde_json::Value,
    pub category: CategoryJson,
    pub shop: ShopJson,
    pub images: Vec<ImageJson>,
    #[serde(rename = "variations")]
    pub variations: Vec<VariationJson>,
}

/// 店铺对象。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShopJson {
    pub uuid: String,
    pub name: String,
    pub subdomain: String,
    pub thumbnail_url: String,
    pub url: String,
    pub verified: bool,
}

/// ID 反序列化：数字或字符串均可。
fn de_id<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::String(s) => Ok(s),
        _ => Ok(String::new()),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CategoryJson {
    pub name: String,
    pub parent: Option<ParentCategory>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ParentCategory {
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ImageJson {
    pub original: String,
    pub resized: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct VariationJson {
    pub price: Option<i64>,
    pub downloadable: Option<DownloadableJson>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DownloadableJson {
    #[serde(rename = "no_musics")]
    pub no_musics: Vec<DownloadFileJson>,
    pub musics: Vec<DownloadFileJson>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DownloadFileJson {
    pub url: String,
    pub name: String,
    pub file_name: String,
    pub file_extension: String,
    /// 人类可读大小（如 "247 KB"），JSON API 为字符串。
    pub file_size: String,
}

/// 解析价格：数字直用 / 字符串剥非数字 / 其他 -1。
pub fn parse_price(price: &serde_json::Value) -> i64 {
    match price {
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(-1),
        serde_json::Value::String(s) => {
            let nums: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
            nums.parse().unwrap_or(0)
        }
        _ => -1,
    }
}

/// 取缩略图：images[0].original 优先，退回 resized。
pub fn thumb_from_json(item: &ItemJson) -> String {
    if let Some(first) = item.images.first() {
        if !first.original.is_empty() {
            return first.original.clone();
        }
        return first.resized.clone();
    }
    String::new()
}

/// 获取商品 JSON（transport 错误重试，HTTP 状态码留给调用方）。
pub fn fetch_item(client: &Client, item_id: &str) -> Result<ItemJson, reqwest::Error> {
    let url = ITEM_JSON_URL.replace("{id}", item_id);
    let mut headers = crate::session::default_headers();
    headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());
    let r = get(client, &url, headers)?;
    r.json()
}

/// 免费可下载文件列表：[(url, filename)]。
///
/// 规则：跳过付费 variation（价格非 0），遍历 no_musics + musics 两组文件。
pub fn free_downloads(item: &ItemJson) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for v in &item.variations {
        if v.price.unwrap_or(0) != 0 {
            continue;
        }
        if let Some(dl) = &v.downloadable {
            for group in [&dl.no_musics, &dl.musics] {
                for f in group {
                    if !f.url.is_empty() {
                        // 文件名优先 name；退回 file_name；再退回占位。
                        let name = if !f.name.is_empty() {
                            f.name.clone()
                        } else if !f.file_name.is_empty() {
                            f.file_name.clone()
                        } else {
                            format!("file_{}", out.len())
                        };
                        out.push((f.url.clone(), name));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_price_variants() {
        assert_eq!(parse_price(&serde_json::json!(800)), 800);
        assert_eq!(parse_price(&serde_json::json!("¥ 1,200")), 1200);
        assert_eq!(parse_price(&serde_json::json!("")), 0);
        assert_eq!(parse_price(&serde_json::json!(null)), -1);
    }

    #[test]
    fn thumb_prefers_original() {
        let item = ItemJson {
            images: vec![
                ImageJson {
                    original: "https://orig".to_string(),
                    resized: "https://resized".to_string(),
                },
                ImageJson {
                    original: "https://orig2".to_string(),
                    resized: "".to_string(),
                },
            ],
            ..ItemJson::default()
        };
        assert_eq!(thumb_from_json(&item), "https://orig");
    }

    #[test]
    fn thumb_empty_when_no_images() {
        assert_eq!(thumb_from_json(&ItemJson::default()), "");
    }

    #[test]
    fn description_defaults_empty() {
        let item: ItemJson = serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": "n"
        }))
        .unwrap();
        assert!(item.description.is_empty());
        let item: ItemJson = serde_json::from_value(serde_json::json!({
            "id": "2",
            "name": "n",
            "description": "条款原文"
        }))
        .unwrap();
        assert_eq!(item.description, "条款原文");
    }

    #[test]
    fn free_downloads_filters_paid_and_collects() {
        let item = ItemJson {
            variations: vec![
                VariationJson {
                    price: Some(500),
                    downloadable: Some(DownloadableJson {
                        no_musics: vec![DownloadFileJson {
                            url: "https://paid".to_string(),
                            name: "paid.zip".to_string(),
                            ..DownloadFileJson::default()
                        }],
                        musics: vec![],
                    }),
                },
                VariationJson {
                    price: Some(0),
                    downloadable: Some(DownloadableJson {
                        no_musics: vec![DownloadFileJson {
                            url: "https://free1".to_string(),
                            name: "free1.zip".to_string(),
                            ..DownloadFileJson::default()
                        }],
                        musics: vec![DownloadFileJson {
                            url: "https://free2".to_string(),
                            name: "".to_string(),
                            ..DownloadFileJson::default()
                        }],
                    }),
                },
            ],
            ..ItemJson::default()
        };
        let files = free_downloads(&item);
        assert_eq!(
            files,
            vec![
                ("https://free1".to_string(), "free1.zip".to_string()),
                ("https://free2".to_string(), "file_1".to_string()),
            ]
        );
    }
}
