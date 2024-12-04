use bt_hci::{controller::ExternalController, WriteHci};
use cyw43::{bluetooth::BtDriver, Control, NetDriver};
use cyw43_pio::PioSpi;
use defmt::{error, info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::{
    join::join3,
    select::{select, Either},
};
use embassy_rp::{
    bind_interrupts,
    gpio::{Level, Output},
    peripherals::{DMA_CH0, PIO0},
    pio::{self, Pio},
};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;
use trouble_host::{prelude::*, Address, Controller, HostResources, PacketQos};

use crate::resources::BltResources;

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

// Battery service
#[gatt_service(uuid = "180f")]
struct BatteryService {
    #[characteristic(uuid = "2a19", read, write, notify, on_read = battery_level_on_read, on_write = battery_level_on_write)]
    level: u8,
}

fn battery_level_on_read(_connection: &Connection) {
    info!("[gatt] Read event on battery level characteristic");
}

fn battery_level_on_write(_connection: &Connection, data: &[u8]) -> Result<(), ()> {
    info!(
        "[gatt] Write event on battery level characteristic: {:?}",
        data
    );
    Ok(())
}

#[gatt_server]
struct Server {
    battery_service: BatteryService,
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

    static STATE: StaticCell<cyw43::State> = StaticCell::new(); // Static means it stays in memory for the lifetime of the program
    let state = STATE.init(cyw43::State::new()); // Initialize the state

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
            name: "TrouBLE",
            appearance: &appearance::GENERIC_POWER,
        }),
    )
    .unwrap();

    let ble_background_task = select(ble_task(runner), gatt_task(&server));

    let app_task = async {
        loop {
            match advertise("Trouble Example", &mut peripheral).await {
                Ok(conn) => {
                    // set up tasks when the connection is established to a central, so they don't run when no one is connected.
                    let connection_task = conn_task(&server, &conn);
                    // let counter_task = counter_task(&server, &conn);
                    // run until any task ends (usually because the connection has been closed),
                    // then return to advertising state.
                    // connection_task.await;
                }
                Err(e) => {
                    info!("Error advertising: {:?}", e);
                    break;
                }
            }
        }
    };

    info!("Starting advertising and GATT service");
    let a = select(ble_background_task, app_task).await;
    match a {
        Either::First(anime) => {
            info!("ble_background_task finished");
        }
        Either::Second(_) => {
            info!("app_task finished");
        }
    }
}

async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) -> Result<(), BleHostError<C::Error>> {
    let a = runner.run().await;
    match &a {
        Ok(_) => {}
        Err(e) => match e {
            BleHostError::Controller(e) => {
                info!("ble controller error")
            }
            BleHostError::BleHost(error) => {
                info!("BleHostError: {:?}", error);
            }
        },
    }
    a
}

async fn gatt_task<C: Controller>(
    server: &Server<'_, '_, C>,
) -> Result<(), BleHostError<C::Error>> {
    loop {
        match server.next().await {
            Ok(_) => {}
            Err(_) => {}
        }
    }
}

/// Example task to use the BLE notifier interface.
async fn counter_task<C: Controller>(server: &Server<'_, '_, C>, conn: &Connection<'_>) {
    let mut tick: u8 = 0;
    let level = server.battery_service.level;
    loop {
        tick = tick.wrapping_add(1);
        info!("[adv] notifying connection of tick {}", tick);
        if server.notify(&level, conn, &tick).await.is_err() {
            info!("[adv] error notifying connection");
            break;
        };
        Timer::after_secs(2).await;
    }
}

async fn conn_task<C: Controller>(
    server: &Server<'_, '_, C>,
    conn: &Connection<'_>,
) -> Result<(), BleHostError<C::Error>> {
    let level = server.battery_service.level;
    while conn.is_connected() {}
    // loop {
    // match conn.next().await {
    //     ConnectionEvent::Disconnected { reason } => {
    //         info!("[gatt] disconnected: {:?}", reason);
    //         break;
    //     }
    //     ConnectionEvent::Gatt { event, .. } => match event {
    //         GattEvent::Read { value_handle } => {
    //             if value_handle == level.handle {
    //                 let value = server.get(&level);
    //                 info!("[gatt] Read Event to Level Characteristic: {:?}", value);
    //             }
    //         }
    //         GattEvent::Write { value_handle } => {
    //             if value_handle == level.handle {
    //                 let value = server.get(&level);
    //                 info!("[gatt] Write Event to Level Characteristic: {:?}", value);
    //             }
    //         }
    //     },
    // }
    // }
    info!("[gatt] task finished");
    Ok(())
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn advertise<'a, C: Controller>(
    name: &'a str,
    peripheral: &mut Peripheral<'a, C>,
) -> Result<Connection<'a>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[Uuid::Uuid16([0x0f, 0x18])]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;
    let mut advertiser = peripheral
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
