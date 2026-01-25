use crate::client::RustyClient;
use crate::config::AccountConfig;
use crate::model::{CalendarListEntry, Task};
use futures::stream::{self, StreamExt};
use std::collections::HashMap;

/// Result for fetching tasks for a single account: `Result<Vec<(CalendarHref, Vec<Task>)>, ErrorString>`
pub type AccountTasksResult = Result<Vec<(String, Vec<Task>)>, String>;

/// Manages multiple `RustyClient` instances keyed by account id.
#[derive(Clone, Debug)]
pub struct ClientManager {
    /// Map of account_id -> RustyClient
    pub clients: HashMap<String, RustyClient>,
}

impl ClientManager {
    pub async fn new(accounts: &[AccountConfig], client_type: Option<&str>) -> Self {
        let mut clients: HashMap<String, RustyClient> = HashMap::new();

        for acc in accounts {
            if acc.url.is_empty() {
                continue;
            }

            match RustyClient::new(
                &acc.url,
                &acc.username,
                &acc.password,
                acc.allow_insecure_certs,
                client_type,
            ) {
                Ok(client) => {
                    clients.insert(acc.id.clone(), client);
                }
                Err(e) => {
                    eprintln!(
                        "Failed to initialize client for account '{}' ({}): {}",
                        acc.name, acc.id, e
                    );
                }
            }
        }

        Self { clients }
    }

    pub fn get_client(&self, account_id: &str) -> Option<&RustyClient> {
        self.clients.get(account_id)
    }

    pub async fn get_all_calendars(&self) -> Vec<CalendarListEntry> {
        let clients = self.clients.clone();

        let futures = clients
            .into_iter()
            .map(|(id, client)| {
                let acc_id = id;
                async move {
                    match client.get_calendars().await {
                        Ok(mut cals) => {
                            for c in &mut cals {
                                c.account_id = acc_id.clone();
                            }
                            cals
                        }
                        Err(_) => Vec::new(),
                    }
                }
            });

        let results: Vec<Vec<CalendarListEntry>> = stream::iter(futures)
            .buffer_unordered(4)
            .collect()
            .await;

        let mut all = Vec::new();
        for mut v in results {
            all.append(&mut v);
        }
        all
    }

    pub async fn get_all_tasks(
        &self,
        calendars: &[CalendarListEntry],
    ) -> Result<Vec<(String, Vec<Task>)>, String> {
        let mut by_account: HashMap<String, Vec<CalendarListEntry>> = HashMap::new();
        for cal in calendars {
            if cal.href.starts_with("local://") {
                continue;
            }
            by_account
                .entry(cal.account_id.clone())
                .or_default()
                .push(cal.clone());
        }

        let clients_map = self.clients.clone();

        let futures = by_account.into_iter().map(|(acc_id, cals)| {
            let client_opt = clients_map.get(&acc_id).cloned();
            let acc_id_clone = acc_id.clone();

            async move {
                match client_opt {
                    Some(client) => match client.get_all_tasks(&cals).await {
                        Ok(res) => Ok(res),
                        Err(e) => Err(format!("Account {}: {}", acc_id_clone, e)),
                    },
                    None => Err(format!("Account {}: offline", acc_id_clone)),
                }
            }
        });

        // Use the type alias here to fix clippy warning
        let results: Vec<AccountTasksResult> = stream::iter(futures)
            .buffer_unordered(4)
            .collect()
            .await;

        let mut all_results: Vec<(String, Vec<Task>)> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        for res in results {
            match res {
                Ok(mut tasks) => all_results.append(&mut tasks),
                Err(e) => errors.push(e),
            }
        }

        if all_results.is_empty() && !errors.is_empty() {
            Err(errors.join("; "))
        } else {
            Ok(all_results)
        }
    }
}
