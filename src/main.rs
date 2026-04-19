use dotenvy::dotenv;
use msisdn_extract_rs::{
    api, at_command,
    device_map::{DeviceMap, ModemInfo},
    scanner, sms,
};
use rand::seq::IndexedRandom;
use std::{error::Error, sync::Arc, time::Instant};

fn add_modem(port: &str, imei: Option<String>, modems: Arc<DeviceMap>) {
    if let Some(imei) = imei {
        modems.insert(
            imei.clone(),
            ModemInfo {
                port: port.into(),
                imei,
                last_seen: Instant::now(),
            },
        );
    }
}

pub fn get_random_msisdn(numbers: &[String]) -> Option<&String> {
    let mut rng = rand::rng();

    numbers.choose(&mut rng)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    let modems = Arc::new(DeviceMap::new());
    let ports = scanner::scan_ports();

    println!("found  ports {:?}", ports);

    for p in &ports {
        match scanner::probe_port(&p).await {
            Some((port, imei)) => {
                add_modem(port, imei, modems.clone());
            }
            None => {
                println!("skipping port {}", p);
            }
        };
    }

    let msisdns = api::get_active_msisdns().await.expect("fetch numbers");

    for modem in modems.iter() {
        if msisdns.len() == 0 {
            eprintln!("cannot find available sms receiver");
            continue;
        }

        let reciver_msisdn = get_random_msisdn(&msisdns).unwrap();

        let Some(icc_id) = at_command::get_iccid(&modem.port).await else {
            eprintln!("cannot read the iccid on the port {}", modem.port);
            continue;
        };

        let Some(_) = at_command::wait_for_gsm_network(&modem.port).await else {
            eprintln!("the modem cannot connected to the gsm network");
            continue;
        };

        let sms_content = format!("iccid is:'{}'", &icc_id);
        let Some(()) = sms::send_sms(&modem.port, &reciver_msisdn, Some(sms_content)).await else {
            eprintln!("cannot send the sms to the number {}", &reciver_msisdn);
            continue;
        };

        println!(
            "the sms has been sent to {}, for iccID: {} on port {}",
            &reciver_msisdn, &icc_id, &modem.port
        );

        let Some(msisdn) = api::read_remote_sms(&reciver_msisdn, &icc_id).await else {
            eprintln!("the sms cannot be read");
            continue;
        };

        if let Some((result, status)) = api::register_msisdn(&msisdn, &icc_id).await {
            if status == 201 {
                println!("{}", "-".repeat(20));
                println!("{}", result);
                println!("{}", "-".repeat(20));
            } else {
                println!("{}", "-".repeat(20));
                println!("msisdn register has been failed due to an error");
                println!("status: {}", status);
                println!("response: {}", result);
                println!("{}", "-".repeat(20));
            }
        } else {
            eprintln!("error on creating the gsm number");
        }
    }

    Ok(())
}
