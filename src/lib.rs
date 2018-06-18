#[macro_use]
extern crate cpython;
extern crate envy;
#[macro_use]
extern crate lando;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate failure;
extern crate reqwest;

use failure::Fail;
use reqwest::header::{Accept, Authorization};
use reqwest::Client;

// extends http::Request type with api gateway info
use lando::RequestExt;

#[derive(Deserialize, Debug)]
struct CommandRequest {
    /// url to respond to slack command with
    response_url: String,
    /// user provided text
    text: String,
}

#[derive(Deserialize)]
struct Config {
    /// pager duty api token
    pd_token: String,
}

#[derive(Deserialize, Debug)]
struct Incidents {
    incidents: Vec<Incident>,
}

#[derive(Deserialize, Debug)]
struct Incident {
    incident_number: usize,
    created_at: String,
    title: String,
    status: String,
    urgency: String,
    html_url: String,
    service: Service,
    assignments: Vec<Assignment>,
}

#[derive(Deserialize, Debug)]
struct Service {
    summary: String,
}

#[derive(Deserialize, Debug)]
struct Assignment {
    at: String,
    assignee: Assignee,
}

#[derive(Deserialize, Debug)]
struct Assignee {
    summary: String,
}

fn incident_response(token: &str) -> reqwest::Result<String> {
    Client::new()
        .get("https://api.pagerduty.com/incidents?statuses%5B%5D=triggered&statuses%5B%5D=acknowledged")
        .header(Accept(vec![
            "application/vnd.pagerduty+json;version=2".parse().unwrap(),
        ]))
        .header(Authorization(format!("Token token={}", token)))
        .send()
        .and_then(|mut r| r.json()).map(format_response)
}

fn format_response(response: Incidents) -> String {
    if response.incidents.is_empty() {
        return String::from("no triggered or acknowledged incidents :thumbsup:");
    }
    let lines = response.incidents.iter().map(|incident| {
        vec![
            format!(
                ":bell: <{link}|{number}> {title}",
                link = incident.html_url,
                number = incident.incident_number,
                title = incident.title
            ),
            format!(
                "> ({status}) {urgency} priority assigned to *{assignee}* for *{service}*",
                status = incident.status,
                urgency = incident.urgency,
                assignee = incident
                    .assignments
                    .iter()
                    .nth(0)
                    .map(|a| a.assignee.summary.clone())
                    .unwrap_or_else(|| "nobody".to_string()),
                service = incident.service.summary
            ),
        ].join("\n")
    });
    lines.collect::<Vec<_>>().join("\n")
}

gateway!(|request, _| {
    let config = envy::from_env::<Config>().map_err(|e| format!("{}", e))?;
    let url = request
        .payload::<CommandRequest>()
        .map_err(|s| s.compat())?
        .unwrap()
        .response_url;
    Client::new()
        .post(&url)
        .json(&json!({ "text": incident_response(&config.pd_token)? }))
        .send()?;
    Ok(lando::Response::new(()))
});
