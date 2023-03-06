use std::{
    fmt::Display,
    fs::{self},
    path::{Path, PathBuf},
};

use reqwest::Url;
use serde::{de::Error, Deserialize, Deserializer};

use crate::error::{SettingsError, SettingsResult};

/// The git repository holding the codebase.
#[derive(Deserialize, Clone)]
pub struct Repository {
    /// The url of the repository.
    #[serde(deserialize_with = "parse_url")]
    pub url: Url,
    /// The commit (or branch name) to deploy.
    pub commit: String,
}

fn parse_url<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    let url = Url::parse(&s).map_err(D::Error::custom)?;

    match url
        .path_segments()
        .map(|x| x.collect::<Vec<_>>().len() >= 2)
    {
        None | Some(false) => Err(D::Error::custom(SettingsError::InvalidRepositoryUrl(
            url.clone(),
        ))),
        _ => Ok(url),
    }
}

/// The list of supported cloud providers.
#[derive(Deserialize, Clone)]
pub enum CloudProvider {
    #[serde(alias = "aws")]
    Aws,
    #[serde(alias = "vultr")]
    Vultr,
}

/// The testbed settings. Those are topically specified in a file.
#[derive(Deserialize, Clone)]
pub struct Settings {
    /// The testbed unique id. This allows multiple users to run concurrent testbeds on the
    /// same cloud provider's account without interference with each others.
    pub testbed_id: String,
    /// The cloud provider hosting the testbed.
    pub cloud_provider: CloudProvider,
    /// The path to the secret token for authentication with the cloud provider.
    pub token_file: PathBuf,
    /// The ssh private key to access the instances.
    pub ssh_private_key_file: PathBuf,
    /// The corresponding ssh public key registered on the instances. If not specified. the
    /// public key defaults the same path as the private key with an added extension 'pub'.
    pub ssh_public_key_file: Option<PathBuf>,
    /// The list of cloud provider regions to deploy the testbed.
    pub regions: Vec<String>,
    /// The specs of the instances to deploy. Those are dependent on the cloud provider, e.g.,
    /// specifying 't3.medium' creates instances with 2 vCPU and 4GBo of ram on AWS.
    pub specs: String,
    /// The details of the git reposit to deploy.
    pub repository: Repository,
    /// The directory where to save benchmarks measurements.
    pub results_directory: PathBuf,
    /// The directory where to download logs files from the instances.
    pub logs_directory: PathBuf,
}

impl Settings {
    /// Load the settings from a json file.
    pub fn load<P>(path: P) -> SettingsResult<Self>
    where
        P: AsRef<Path> + Display + Clone,
    {
        let reader = || -> Result<Self, std::io::Error> {
            let data = fs::read(path.clone())?;
            let settings: Settings = serde_json::from_slice(data.as_slice())?;

            fs::create_dir_all(&settings.results_directory)?;
            fs::create_dir_all(&settings.logs_directory)?;

            Ok(settings)
        };

        reader().map_err(|e| SettingsError::InvalidSettings {
            file: path.to_string(),
            message: e.to_string(),
        })
    }

    /// Get the name of the repository (from its url).
    pub fn repository_name(&self) -> String {
        self.repository
            .url
            .path_segments()
            .expect("Url should already be checked when loading settings")
            .collect::<Vec<_>>()[1]
            .to_string()
    }

    /// Load the secret token to authenticate with the cloud provider.
    pub fn load_token(&self) -> SettingsResult<String> {
        match fs::read_to_string(&self.token_file) {
            Ok(token) => Ok(token.trim_end_matches('\n').to_string()),
            Err(e) => Err(SettingsError::InvalidTokenFile {
                file: self.token_file.display().to_string(),
                message: e.to_string(),
            }),
        }
    }

    /// Load the ssh public key from file.
    pub fn load_ssh_public_key(&self) -> SettingsResult<String> {
        let ssh_public_key_file = self.ssh_public_key_file.clone().unwrap_or_else(|| {
            let mut private = self.ssh_private_key_file.clone();
            private.set_extension("pub");
            private
        });
        match fs::read_to_string(&ssh_public_key_file) {
            Ok(token) => Ok(token.trim_end_matches('\n').to_string()),
            Err(e) => Err(SettingsError::InvalidSshPublicKeyFile {
                file: ssh_public_key_file.display().to_string(),
                message: e.to_string(),
            }),
        }
    }

    /// The number of regions specified in the settings.
    #[cfg(test)]
    pub fn number_of_regions(&self) -> usize {
        self.regions.len()
    }

    /// Test settings for unit tests.
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self {
            testbed_id: "testbed".into(),
            cloud_provider: CloudProvider::Aws,
            token_file: "/path/to/token/file".into(),
            ssh_private_key_file: "/path/to/private/key/file".into(),
            ssh_public_key_file: None,
            regions: vec!["London".into(), "New York".into()],
            specs: "small".into(),
            repository: Repository {
                url: Url::parse("https://example.net/author/repo").unwrap(),
                commit: "main".into(),
            },
            results_directory: "results".into(),
            logs_directory: "logs".into(),
        }
    }
}

#[cfg(test)]
mod test {
    use reqwest::Url;

    use crate::settings::Settings;

    #[test]
    fn repository_name() {
        let mut settings = Settings::new_for_test();
        settings.repository.url = Url::parse("https://example.com/author/repo").unwrap();
        assert_eq!(settings.repository_name(), "repo");
    }
}