mod common;
use common::{create_port, Port};
use std::convert::TryFrom;

use bluerobotics_ping::{
    device::{Ping1Dtsr, PingDevice},
    error::PingError,
    message::MessageInfo,
    ping1dtsr::{self, ProfileStruct},
    Messages,
};

#[tokio::main]
async fn main() -> Result<(), PingError> {
    println!("Parsing user provided values and creating port...");
    let port = create_port().await;

    println!("Creating your Ping 1D device");
    let ping1dtsr = match port {
        Port::Serial(port) => Ping1Dtsr::new(port),
        Port::Udp(port) => Ping1Dtsr::new(port),
    };

    // Creating a subscription channel which will receive 30 Profile measurements, we'll check this after the next methods!
    let mut subscribed = ping1dtsr.subscribe();
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<ProfileStruct>>();
    ping1dtsr
        .continuous_start(bluerobotics_ping::ping1dtsr::ProfileStruct::id())
        .await?;

    tokio::spawn(async move {
        let mut profile_struct_vector: Vec<ProfileStruct> = Vec::new();
        loop {
            let received = subscribed.recv().await;
            match received {
                Ok(msg) => {
                    let msgid = msg.message_id;
                    let msgid2 = bluerobotics_ping::ping1dtsr::ProfileStruct::id(); 
                    println!("Got message with ID: {msgid}");
                    if msg.message_id == bluerobotics_ping::ping1dtsr::ProfileStruct::id() {
                        println!("Found profile message with ID: {msgid2}");
                        match Messages::try_from(&msg) {
                            Ok(Messages::Ping1Dtsr(ping1dtsr::Messages::Profile(answer))) => {
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
/*
    for n in (1..10).rev() {
        println!("Testing set/get device id: {n}");
        ping1dtsr.set_device_id(n).await?;
        assert_eq!(n, ping1dtsr.device_id().await.unwrap().device_id);
    }
*/
    // Testing set command, all set commands check for their Ack message, Error and NAck error are possible
    println!(
        "Set gain to auto: {:?}",
        ping1dtsr.set_mode_auto(1).await.is_ok()
    );
    ping1dtsr.set_speed_of_sound(343000).await?;
    let mut speed_of_sound_struct = ping1dtsr.speed_of_sound().await?;
    println!(
        "Test set & get with a new speed of sound: {:?} m/s",
        speed_of_sound_struct.speed_of_sound as f64 / 1000.0
    );
    ping1dtsr.set_speed_of_sound(1500000).await?;
    speed_of_sound_struct = ping1dtsr.speed_of_sound().await?;
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
        distance_struct,
    ) = tokio::try_join!(
        ping1dtsr.protocol_version(),
        ping1dtsr.device_id(),
        ping1dtsr.gain_setting(),
        ping1dtsr.processor_temperature(),
        ping1dtsr.voltage_5(),
        ping1dtsr.distance(),
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

    println!("Turning-off the continuous messages stream from Ping1Dtsr");
    ping1dtsr
        .continuous_stop(bluerobotics_ping::ping1dtsr::ProfileStruct::id())
        .await?;

    Ok(())
}
