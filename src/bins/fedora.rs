use url::Url;
use hyper::Client;
use serde_json;

use lib::*;
use lib::Result;
use lib::error::*;
use lib::files::*;

use std::io::Read;

pub struct Fedora {
  client: Client
}

impl Fedora {
  pub fn new() -> Fedora {
    Fedora {
      client: ::new_client()
    }
  }

  fn id_from_url(&self, url: &str) -> Option<String> {
    let url = option!(Url::parse(url).ok());
    let mut segments = option!(url.path_segments());
    let id_segment = segments.nth(1);
    id_segment.map(|x| x.to_owned())
  }
}

impl Bin for Fedora {
  fn name(&self) -> &str {
    "fedora"
  }

  fn html_host(&self) -> &str {
    "paste.fedoraproject.org"
  }

  fn raw_host(&self) -> &str {
    "paste.fedoraproject.org"
  }
}

impl ManagesUrls for Fedora {}

impl CreatesUrls for Fedora {}

impl FormatsUrls for Fedora {}

impl FormatsHtmlUrls for Fedora {
  fn format_html_url(&self, id: &str) -> Option<String> {
    Some(format!("https://paste.fedoraproject.org/paste/{}", id))
  }
}

impl FormatsRawUrls for Fedora {
  fn format_raw_url(&self, id: &str) -> Option<String> {
    Some(format!("https://paste.fedoraproject.org/paste/{}/raw", id))
  }
}

impl CreatesHtmlUrls for Fedora {
  fn create_html_url(&self, id: &str) -> Result<Vec<PasteUrl>> {
    let html_url = self.format_html_url(id).unwrap();
    let raw_url = self.format_raw_url(id).unwrap();
    let mut res = self.client.get(&raw_url).send()?;
    let mut content = String::new();
    res.read_to_string(&mut content)?;
    if res.status.class().default_code() != ::hyper::Ok {
      debug!("bad status code");
      return Err(ErrorKind::InvalidStatus(res.status_raw().0, Some(content)).into());
    }
    let parsed: serde_json::Result<Vec<IndexedFile>> = serde_json::from_str(&content);
    match parsed {
      Ok(is) => {
        debug!("file was an index, so checking its urls");
        let ids: Option<Vec<(String, String)>> = is.iter().map(|x| self.id_from_html_url(&x.url).map(|i| (x.name.clone(), i))).collect();
        let ids = match ids {
          Some(i) => i,
          None => {
            debug!("could not parse an ID from one of the URLs in the index");
            bail!("one of the URLs in the index did not contain a valid ID");
          }
        };
        Ok(ids.into_iter().map(|(name, id)| PasteUrl::raw(Some(PasteFileName::Explicit(name)), self.format_html_url(&id).unwrap())).collect())
      },
      Err(_) => Ok(vec![PasteUrl::Downloaded(html_url, DownloadedFile::new(PasteFileName::Guessed(id.to_owned()), content))])
    }
  }

  fn id_from_html_url(&self, url: &str) -> Option<String> {
    self.id_from_url(url)
  }
}

impl CreatesRawUrls for Fedora {
  fn create_raw_url(&self, id: &str) -> Result<Vec<PasteUrl>> {
    debug!("creating raw url for {}", id);
    let raw_url = self.format_raw_url(id).unwrap();
    let mut res = self.client.get(&raw_url).send()?;
    let mut content = String::new();
    res.read_to_string(&mut content)?;
    if res.status.class().default_code() != ::hyper::Ok {
      debug!("bad status code");
      return Err(ErrorKind::InvalidStatus(res.status_raw().0, Some(content)).into());
    }
    let parsed: serde_json::Result<Vec<IndexedFile>> = serde_json::from_str(&content);
    match parsed {
      Ok(is) => {
        debug!("file was an index, so checking its urls");
        let ids: Option<Vec<(String, String)>> = is.iter().map(|x| self.id_from_html_url(&x.url).map(|i| (x.name.clone(), i))).collect();
        let ids = match ids {
          Some(i) => i,
          None => {
            debug!("could not parse an ID from one of the URLs in the index");
            bail!("one of the URLs in the index did not contain a valid ID");
          }
        };
        Ok(ids.into_iter().map(|(name, id)| PasteUrl::raw(Some(PasteFileName::Explicit(name)), self.format_raw_url(&id).unwrap())).collect())
      },
      Err(_) => Ok(vec![PasteUrl::Downloaded(raw_url, DownloadedFile::new(PasteFileName::Guessed(id.to_owned()), content))])
    }
  }

  fn id_from_raw_url(&self, url: &str) -> Option<String> {
    self.id_from_url(url)
  }
}

impl HasFeatures for Fedora {
  fn features(&self) -> Vec<BinFeature> {
    vec![BinFeature::Public,
         BinFeature::Anonymous,
         BinFeature::SingleNaming]
  }
}

impl UploadsSingleFiles for Fedora {
  fn upload_single(&self, file: &UploadFile) -> Result<PasteUrl> {
    debug!("uploading single file");
    let params = FedoraParams {
      contents: file.content.clone(),
      title: file.name.clone()
    };
    let params_json = serde_json::to_string(&params)?;
    let mut res = self.client.post("https://paste.fedoraproject.org/api/paste/submit")
      .header(::hyper::header::ContentType::json())
      .body(&params_json)
      .send()?;
    debug!("res: {:?}", res);
    let mut content = String::new();
    res.read_to_string(&mut content)?;
    debug!("content: {}", content);
    if res.status.class().default_code() != ::hyper::Ok {
      debug!("bad status code");
      return Err(ErrorKind::InvalidStatus(res.status_raw().0, Some(content)).into());
    }
    let response: FedoraResponse = serde_json::from_str(&content)?;
    debug!("parse: {:?}", response);
    if let Some(false) = response.success {
      debug!("upload was a failure");
      return Err(ErrorKind::BinError(response.message.unwrap_or_else(|| String::from("<no error given>"))).into());
    } else {
      debug!("upload was a success. creating html url");
      let url = response.url;
      return Ok(PasteUrl::html(Some(PasteFileName::Explicit(file.name.clone())), url));
    }
  }
}

impl HasClient for Fedora {
  fn client(&self) -> &Client {
    &self.client
  }
}

#[derive(Debug, Serialize)]
struct FedoraParams {
  contents: String,
  title: String
}

#[derive(Debug, Deserialize)]
struct FedoraResponse {
  // key only appears for error results. thanks, lying documentation
  #[serde(default)]
  success: Option<bool>,
  message: Option<String>,
  url: String
}
