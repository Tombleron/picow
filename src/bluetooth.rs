use core::sync::atomic::Ordering;

use bt_hci::controller::ExternalController;
use cyw43_pio::PioSpi;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_rp::{
    bind_interrupts,
    gpio::{Level, Output},
    peripherals::{DMA_CH0, PIO0},
    pio::{self, Pio},
};
use embassy_time::Timer;
use static_cell::StaticCell;
use trouble_host::{prelude::*, Address, Controller, HostResources, PacketQos};

use crate::{
    emg::{EMG1_VALUE, EMG2_VALUE},
    resources::BltResources,
};

bind_interrupts!(struct BltIrqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

// Async task for managing the CYW43 Wi-Fi chip
#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await // Runs the Wi-Fi task indefinitely
}

/// Size of L2CAP packets (ATT MTU is this - 4)
const L2CAP_MTU: usize = 251;
/// Max number of connections
const CONNECTIONS_MAX: usize = 1;
/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att
const MAX_ATTRIBUTES: usize = 10;
type Resources<C> = HostResources<C, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU>;

// Define the service for prosthetic arm
#[gatt_service(uuid = "1815")]
struct ProstheticArmService {
    #[characteristic(uuid = "2A58", read, notify)]
    erm_sensor_1: u16,

    #[characteristic(uuid = "2A59", read, notify)]
    erm_sensor_2: u16,

    #[characteristic(uuid = "7345", read, write)]
    sensitivity_min_1: u16,

    #[characteristic(uuid = "7346", read, write)]
    sensitivity_max_1: u16,

    #[characteristic(uuid = "7347", read, write)]
    sensitivity_min_2: u16,

    #[characteristic(uuid = "7348", read, write)]
    sensitivity_max_2: u16,
}

fn erm_sensor_1_on_read(_connection: &Connection) {
    info!("[gatt] Read event on ERM sensor 1");
}

fn erm_sensor_2_on_read(_connection: &Connection) {
    info!("[gatt] Read event on ERM sensor 2");
}

fn sensitivity_min_1_on_read(_connection: &Connection) {
    info!("[gatt] Read event on sensitivity minimum 1");
}

fn sensitivity_min_1_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    if data.len() == 2 {
        let value = u16::from_le_bytes([data[0], data[1]]);
        info!("[gatt] New sensitivity minimum 1: {}", value);
        Ok(())
    } else {
        info!("[gatt] Invalid sensitivity minimum 1 data");
        Err(())
    }
}

fn sensitivity_max_1_on_read(_connection: &Connection) {
    info!("[gatt] Read event on sensitivity maximum 1");
}

fn sensitivity_max_1_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    if data.len() == 2 {
        let value = u16::from_le_bytes([data[0], data[1]]);
        info!("[gatt] New sensitivity maximum 1: {}", value);
        Ok(())
    } else {
        info!("[gatt] Invalid sensitivity maximum 1 data");
        Err(())
    }
}

fn sensitivity_min_2_on_read(_connection: &Connection) {
    info!("[gatt] Read event on sensitivity minimum 2");
}

fn sensitivity_min_2_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    if data.len() == 2 {
        let value = u16::from_le_bytes([data[0], data[1]]);
        info!("[gatt] New sensitivity minimum 2: {}", value);
        Ok(())
    } else {
        info!("[gatt] Invalid sensitivity minimum 2 data");
        Err(())
    }
}

fn sensitivity_max_2_on_read(_connection: &Connection) {
    info!("[gatt] Read event on sensitivity maximum 2");
}

fn sensitivity_max_2_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    if data.len() == 2 {
        let value = u16::from_le_bytes([data[0], data[1]]);
        info!("[gatt] New sensitivity maximum 2: {}", value);
        Ok(())
    } else {
        info!("[gatt] Invalid sensitivity maximum 2 data");
        Err(())
    }
}

#[gatt_server]
struct Server {
    prosthetic_arm_service: ProstheticArmService,
}

#[embassy_executor::task]
pub async fn initialize_bluetooth(spawner: Spawner, p: BltResources) -> () {
    let fw = include_bytes!("firmware/43439A0.bin");
    let clm = include_bytes!("firmware/43439A0_clm.bin");
    let btfw = include_bytes!("firmware/43439A0_btfw.bin");

    let pwr = Output::new(p.pwr, Level::Low);
    let cs = Output::new(p.cs, Level::High);

    let mut pio = Pio::new(p.pio, BltIrqs);
    let spi = PioSpi::new(&mut pio.common, pio.sm0, pio.irq0, cs, p.dio, p.clk, p.dma);

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());

    let (_net_device, bt_device, mut control, runner) =
        cyw43::new_with_bluetooth(state, pwr, spi, fw, btfw).await;
    unwrap!(spawner.spawn(cyw43_task(runner)));
    control.init(clm).await;

    let controller: ExternalController<_, 10> = ExternalController::new(bt_device);

    let address = Address::random([0x41, 0x5A, 0xE3, 0x1E, 0x83, 0xE7]);
    info!("Our address = {:?}", address);

    let mut resources = Resources::new(PacketQos::None);
    let (stack, mut peripheral, _, runner) = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .build();

    let server = Server::new_with_config(
        stack,
        GapConfig::Peripheral(PeripheralConfig {
            name: "ProstheticArm",
            appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
        }),
    )
    .unwrap();

    let ble_background_task = select(ble_task(runner), gatt_task(&server));

    let app_task = async {
        loop {
            match advertise("ProstheticArm", &mut peripheral).await {
                Ok(conn) => {
                    let connection_task = conn_task(&server, &conn);
                    let sensor_task = sensor_update_task(&server, &conn);
                    select(connection_task, sensor_task).await;
                }
                Err(e) => {
                    info!("Error advertising: {:?}", e);
                    break;
                }
            }
        }
    };

    info!("Starting advertising and GATT service");
    select(ble_background_task, app_task).await;
}

async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) -> Result<(), BleHostError<C::Error>> {
    loop {
        if let Err(_e) = runner.run().await {
            let _e = defmt::Debug2Format(&_e);
        }
    }
}

async fn gatt_task<C: Controller>(
    server: &Server<'_, '_, C>,
) -> Result<(), BleHostError<C::Error>> {
    loop {
        if let Err(_e) = server.run().await {
            let _e = defmt::Debug2Format(&_e);
        }
    }
}

async fn sensor_update_task<C: Controller>(server: &Server<'_, '_, C>, conn: &Connection<'_>) {
    let erm1 = server.prosthetic_arm_service.erm_sensor_1;
    let erm2 = server.prosthetic_arm_service.erm_sensor_2;

    loop {
        let sensor1_value: u16 = EMG1_VALUE.load(Ordering::Relaxed) as u16;
        let sensor2_value: u16 = EMG2_VALUE.load(Ordering::Relaxed) as u16;

        if server.notify(&erm1, conn, &sensor1_value).await.is_err() {
            info!("[adv] error notifying ERM1 value");
            break;
        }

        if server.notify(&erm2, conn, &sensor2_value).await.is_err() {
            info!("[adv] error notifying ERM2 value");
            break;
        }

        Timer::after_millis(100).await;
    }
}

async fn conn_task<C: Controller>(
    server: &Server<'_, '_, C>,
    conn: &Connection<'_>,
) -> Result<(), BleHostError<C::Error>> {
    let erm1 = server.prosthetic_arm_service.erm_sensor_1;
    let erm2 = server.prosthetic_arm_service.erm_sensor_2;
    let sens_min1 = server.prosthetic_arm_service.sensitivity_min_1;
    let sens_max1 = server.prosthetic_arm_service.sensitivity_max_1;
    let sens_min2 = server.prosthetic_arm_service.sensitivity_min_2;
    let sens_max2 = server.prosthetic_arm_service.sensitivity_max_2;

    loop {
        match conn.next().await {
            ConnectionEvent::Disconnected { reason } => {
                info!("[gatt] disconnected: {:?}", reason);
                break;
            }
            ConnectionEvent::Gatt { event, .. } => match event {
                GattEvent::Read { value_handle } => {
                    if value_handle == erm1.handle {
                        let value = server.get(&erm1);
                        info!("[gatt] Read ERM1 value: {:?}", value);
                    } else if value_handle == erm2.handle {
                        let value = server.get(&erm2);
                        info!("[gatt] Read ERM2 value: {:?}", value);
                    } else if value_handle == sens_min1.handle {
                        let value = server.get(&sens_min1);
                        info!("[gatt] Read sensitivity min 1: {:?}", value);
                    } else if value_handle == sens_max1.handle {
                        let value = server.get(&sens_max1);
                        info!("[gatt] Read sensitivity max 1: {:?}", value);
                    } else if value_handle == sens_min2.handle {
                        let value = server.get(&sens_min2);
                        info!("[gatt] Read sensitivity min 2: {:?}", value);
                    } else if value_handle == sens_max2.handle {
                        let value = server.get(&sens_max2);
                        info!("[gatt] Read sensitivity max 2: {:?}", value);
                    }
                }
                GattEvent::Write { value_handle } => {
                    if value_handle == sens_min1.handle {
                        let value = server.get(&sens_min1);
                        info!("[gatt] New sensitivity min 1: {:?}", value);
                    } else if value_handle == sens_max1.handle {
                        let value = server.get(&sens_max1);
                        info!("[gatt] New sensitivity max 1: {:?}", value);
                    } else if value_handle == sens_min2.handle {
                        let value = server.get(&sens_min2);
                        info!("[gatt] New sensitivity min 2: {:?}", value);
                    } else if value_handle == sens_max2.handle {
                        let value = server.get(&sens_max2);
                        info!("[gatt] New sensitivity max 2: {:?}", value);
                    }
                }
            },
        }
    }
    info!("[gatt] task finished");
    Ok(())
}

async fn advertise<'a, C: Controller>(
    name: &'a str,
    peripheral: &mut Peripheral<'a, C>,
) -> Result<Connection<'a>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[Uuid::Uuid16([0x15, 0x18])]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..],
                scan_data: &[],
            },
        )
        .await?;
    info!("[adv] advertising");
    let conn = advertiser.accept().await?;
    info!("[adv] connection established");
    Ok(conn)
}
