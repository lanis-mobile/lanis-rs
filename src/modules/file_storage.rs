use tokio::fs::File;
use chrono::{DateTime, Utc};
use markup5ever::interface::TreeSink;
use markup5ever::tendril::fmt::Slice;
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use crate::utils::constants::URL;
use crate::utils::conversion::string_to_byte_size;
use crate::utils::datetime::date_time_string_to_datetime;
use crate::Error;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct FileStoragePage {
    pub folder_nodes: Vec<FolderNode>,
    pub file_nodes: Vec<FileNode>,
}

impl FileStoragePage {
    pub fn new(folder_nodes: Vec<FolderNode>, file_nodes: Vec<FileNode>) -> Self {
        Self { folder_nodes, file_nodes }
    }

    /// Get the root [FileStoragePage]
    pub async fn get_root(client: &Client) -> Result<Self, Error> {
        Self::get_page(client, &[("a", "view")]).await
    }

    /// Get a [FileStoragePage] for a specific folder node
    pub async fn get(node_id: i32, client: &Client) -> Result<Self, Error> {
        Self::get_page(client, &[("a", "view"), ("folder", &node_id.to_string())]).await
    }

    async fn get_page<T: Serialize>(client: &Client, query_parameter: &T) -> Result<Self, Error> {
        match client.get(URL::DATA_STORAGE).query(query_parameter).send().await {
            Ok(response) => {
                async fn string_or_none<'a>(option: Option<ElementRef<'a>>) -> Option<String> {
                    match option {
                        Some(element) => Some(element.text().collect::<String>().trim().to_string()),
                        None => None,
                    }
                }

                let text = response.text().await.map_err(|e| Error::Parsing(format!("failed to parse response as text '{}'", e)))?;
                let html = Html::parse_document(&text);

                let mut folder_nodes: Vec<FolderNode> = Vec::new();

                let folder_selector = Selector::parse(".folder").unwrap();
                let folder_name_selector = Selector::parse(".caption").unwrap();
                let folder_description_selector = Selector::parse(".desc").unwrap();
                let folder_subfolders_selector = Selector::parse("div.row>div.col-md-12>small>span.label.label-info").unwrap();

                let folders = html.select(&folder_selector);
                for folder in folders {
                    let id = folder.attr("data-id").unwrap_or("0").trim().parse::<i32>().map_err(|e| Error::Parsing(format!("failed to parse id of folder node as i32 '{}'", e)))?;
                    let name_future = string_or_none(folder.select(&folder_name_selector).nth(0));
                    let description_future = string_or_none(folder.select(&folder_description_selector).nth(0));
                    let subfolders = string_or_none(folder.select(&folder_subfolders_selector).nth(0)).await.unwrap_or(String::from("0")).replace(" Ordner", "").replace("Keine Dateien", "0").parse::<i32>().map_err(|e| Error::Parsing(format!("failed to parse subfolder count as i32 '{}'", e)))?;

                    let name = match name_future.await {
                        Some(name) => name,
                        None => return Err(Error::Parsing(String::from("failed to parse name of folder node 'name is None'")))
                    };
                    let description = description_future.await;

                    folder_nodes.push(FolderNode::new(id, name, description, subfolders))
                }

                let mut file_nodes: Vec<FileNode> = Vec::new();

                let file_selector = Selector::parse("table#files>tbody>tr").unwrap();
                let td_selector = Selector::parse("td").unwrap();
                let small_selector = Selector::parse("small").unwrap();

                let file_header_selector = Selector::parse("table#files thead th").unwrap();

                let file_headers = html.select(&file_header_selector).map(|element| element.text().collect::<String>().trim().to_string()).collect::<Vec<_>>();

                for file_element in html.select(&file_selector) {
                    let mut file = Html::parse_document(&format!("<body><table><tbody>{}</tbody></table></body>", &file_element.html()));

                    let fields = file.select(&td_selector).collect::<Vec<ElementRef>>();

                    let id = file_element.attr("data-id").unwrap_or("0").trim().parse::<i32>().map_err(|e| Error::Parsing(format!("failed to parse id of file node as i32 '{}'", e)))?;

                    let name_pos = file_headers.iter().position(|r| r == "Name");

                    // TODO: Test this
                    let notice = {
                        match fields.get(name_pos.unwrap_or(fields.len())) {
                            Some(element) => {
                                match element.select(&small_selector).nth(0) {
                                    Some(element) => {
                                        let text = element.text().collect::<String>().trim().to_string();
                                        file.remove_from_parent(&element.id());
                                        Some(text)
                                    },
                                    None => None,
                                }
                            }
                            None => None,
                        }
                    };

                    let fields = file.select(&td_selector).collect::<Vec<ElementRef>>();

                    let name_future = string_or_none(fields.get(name_pos.unwrap_or(fields.len())).copied());

                    let changed_pos = file_headers.iter().position(|r| r == "Änderung");
                    let changed_future = string_or_none(fields.get(changed_pos.unwrap_or(fields.len())).copied());

                    let size_pos = file_headers.iter().position(|r| r == "Größe");
                    let size_future = string_or_none(fields.get(size_pos.unwrap_or(fields.len())).copied());

                    let name = match name_future.await {
                        Some(name) => name,
                        None => return Err(Error::Parsing(String::from("failed to parse name of file node 'name is None'")))
                    };

                    let changed = match changed_future.await {
                        Some(changed) => {
                            let mut split = changed.split(' ');
                            let date = split.nth(0).ok_or_else(|| Error::Parsing(String::from("failed to parse date for file node 'not found'")))?.to_string();
                            let time = split.nth(0).ok_or_else(|| Error::Parsing(String::from("failed to parse time for file node 'not found'")))?.to_string();

                            date_time_string_to_datetime(&date, &time).map_err(|e| Error::DateTime(format!("failed to convert file node changed date & time to DateTime '{:?}'", e)))?.to_utc()
                        },
                        None => DateTime::from_timestamp_nanos(0).into(),
                    };

                    let size = match size_future.await {
                        Some(size) => string_to_byte_size(size).await.map_err(|e| Error::Parsing(format!("failed to convert size into u64 '{:?}'", e)))?,
                        None => return Err(Error::Parsing(String::from("failed to parse size of file node 'size is None'")))
                    };

                    file_nodes.push(FileNode::new(id, name, changed, size, notice));
                }

                Ok(Self { folder_nodes, file_nodes })
            }
            Err(e) => Err(Error::Network(format!("failed to get page: '{}'", e.to_string())))?,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct FileNode {
    pub id: i32,
    pub name: String,
    /// The last time the file was changed
    pub changed: DateTime<Utc>,
    pub size: u64,
    pub notice: Option<String>,
}

impl FileNode {
    pub fn new(id: i32, name: String, changed: DateTime<Utc>, size: u64, notice: Option<String>) -> Self {
        Self { id, name, changed, size, notice }
    }

    /// Downloads the file to the given location. <br>
    /// Please note that the given file path will be overwritten if there is already a file
    pub async fn download(&self, path: &str, client: &Client) -> Result<(), Error> {
        match client.get(URL::DATA_STORAGE).query(&[("a", "download"), ("f", &self.id.to_string())]).send().await {
            Ok(response) => {
                let bytes = response.bytes().await.map_err(|e| Error::Parsing(format!("failed to convert response to bytes '{}'", e)))?.as_bytes().to_vec();

                let mut file = File::create(path).await.map_err(|e| Error::FileSystem(format!("failed to create file in desired path '{}'", e)))?;
                tokio::io::copy(&mut bytes.as_ref(), &mut file).await.map_err(|e| Error::FileSystem(format!("failed to copy downloaded file to desired path '{}'", e)))?;

                Ok(())
            }
            Err(e) => Err(Error::Network(format!("download failed '{}'", e))),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct FolderNode {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    /// The amount of subfolders
    pub subfolders: i32,
}

impl FolderNode {
    pub fn new(id: i32, name: String, description: Option<String>, subfolders: i32) -> Self {
        Self { id, name, description,  subfolders }
    }
}