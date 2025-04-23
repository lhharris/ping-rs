mod common;
use common::{configure_tracing, create_port, Port};
use std::convert::TryFrom;
use tokio::time::{sleep, Duration};

use bluerobotics_ping::{
    device::{Tsr1000, PingDevice},
    error::PingError,
    message::MessageInfo,
    tsr1000::{self, ProfileStruct},
    Messages,
};

#[tokio::main]
async fn main() -> Result<(), PingError> {
    configure_tracing();

    println!("Parsing user provided values and creating port...");
    let port = create_port().await;

    println!("Creating your Ping 1D device");
    let tsr1000 = match port {
        Port::Serial(port) => Tsr1000::new(port),
        Port::Udp(port) => Tsr1000::new(port),
    };

    // Creating a subscription channel which will receive 30 Profile measurements, we'll check this after the next methods!
    let mut subscribed = tsr1000.subscribe();
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<ProfileStruct>>();
    tsr1000
        .continuous_start(bluerobotics_ping::tsr1000::ProfileStruct::id())
        .await?;

    tokio::spawn(async move {
        let mut profile_struct_vector: Vec<ProfileStruct> = Vec::new();
        loop {
            let received = subscribed.recv().await;
            match received {
                Ok(msg) => {
                    if msg.message_id == bluerobotics_ping::tsr1000::ProfileStruct::id() {
                        match Messages::try_from(&msg) {
                            Ok(Messages::Tsr1000(tsr1000::Messages::Profile(answer))) => {
                                profile_struct_vector.push(answer)
                            }
                            _ => continue,
                        }
                    }
                }
                Err(_e) => break,
            }
            if profile_struct_vector.len() >= 30 {
                tx.send(profile_struct_vector).unwrap();
                break;
            };
        }
    });

    for n in (1..10).rev() {
        println!("Testing set/get device id: {n}");
        tsr1000.set_device_id(n).await?;
        assert_eq!(n, tsr1000.device_id().await.unwrap().device_id);
    }

    // Testing set command, all set commands check for their Ack message, Error and NAck error are possible
    println!(
        "Set gain to auto: {:?}",
        tsr1000.set_mode_auto(1).await.is_ok()
    );
    tsr1000.set_speed_of_sound(343000).await?;
    let mut speed_of_sound_struct = tsr1000.speed_of_sound().await?;
    println!(
        "Test set & get with a new speed of sound: {:?} m/s",
        speed_of_sound_struct.speed_of_sound as f64 / 1000.0
    );
    tsr1000.set_speed_of_sound(1500000).await?;
    speed_of_sound_struct = tsr1000.speed_of_sound().await?;
    println!(
        "Test set & get with default speed of sound: {:?} m/s",
        speed_of_sound_struct.speed_of_sound as f64 / 1000.0
    );

    // Creating futures to read different device Properties
    let (
        protocol_version_struct,
        device_id_struct,
        gain_setting_struct,
        processor_temperature_struct,
        voltage5_struct,
    ) = tokio::try_join!(
        tsr1000.protocol_version(),
        tsr1000.device_id(),
        tsr1000.gain_setting(),
        tsr1000.processor_temperature(),
        tsr1000.voltage_5(),
    )
    .expect("Failed to join results");
    let (    
        distance_struct,
    ) = tokio::try_join!(
        tsr1000.distance(),
    )
    .expect("Failed to join results");

    let version = format!(
        "{}.{}.{}",
        protocol_version_struct.version_major,
        protocol_version_struct.version_minor,
        protocol_version_struct.version_patch
    );

    println!("Protocol version is: {version}");
    println!("Device id is: {:?}", device_id_struct.device_id);
    println!("Gain setting is: {:?}", gain_setting_struct.gain_setting);
    println!(
        "Processor temperature is: {:.2} °C",
        processor_temperature_struct.processor_temperature as f64 / 100.0
    );
    println!(
        "Voltage at 5V lane is: {:.3} V",
        voltage5_struct.voltage_5 as f64 / 1000.0
    );
    println!(
        "The distance to target is: {:?} mm",
        distance_struct.distance
    );

    // Read the 30 packages we are waiting since the start of this example, all above tasks have success, we did it!
    println!("Waiting for 30 profiles...");
    match rx.await {
        Ok(v) => println!("Received {} profiles", v.len()),
        Err(_) => println!("The oneshot sender dropped"),
    }

    println!("Turning-off the continuous messages stream from Tsr1000");
    let result = tsr1000
        .continuous_stop(bluerobotics_ping::tsr1000::ProfileStruct::id())
        .await;
    match result {
	Ok(_) => println!("Stopped successfully"),
	Err(e) => println!("Error stopping: {:?}", e),
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    Ok(())
}
