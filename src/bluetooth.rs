use bt_hci::{controller::ExternalController, WriteHci};
use cyw43::{bluetooth::BtDriver, Control, NetDriver};
use cyw43_pio::PioSpi;
use defmt::{error, info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::join::join3;
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
const CONNECTIONS_MAX: usize = 2;
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

#[gatt_server(attribute_data_size = 10)]
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
    let (stack, peripheral, _, runner) = trouble_host::new(controller, &mut resources)
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

    info!("Starting advertising and GATT service");
    let _ = join3(ble_task(runner), gatt_task(&server), async {
        advertise_task(peripheral, &server).await;
        info!("[adv] advertising done")
    })
    .await;
}

async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) -> Result<(), BleHostError<C::Error>> {
    runner.run().await
}

async fn gatt_task<C: Controller>(server: &Server<'_, '_, C>) {
    loop {
        match server.next().await {
            Ok(GattEvent::Write {
                value_handle,
                connection: _,
            }) => {
                info!("[gatt] Write event on {:?}", value_handle);
            }
            Ok(GattEvent::Read {
                value_handle,
                connection: _,
            }) => {
                info!("[gatt] Read event on {:?}", value_handle);
            }
            Err(e) => {
                error!("[gatt] Error processing GATT events: {:?}", e);
            }
        }
    }
}

async fn advertise_task<C: Controller>(
    mut peripheral: Peripheral<'_, C>,
    server: &Server<'_, '_, C>,
) -> Result<(), BleHostError<C::Error>> {
    let mut adv_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            // AdStructure::ServiceUuids16(&[Uuid::Uuid16([0x0f, 0x18])]),
            AdStructure::CompleteLocalName(b"Trouble"),
        ],
        &mut adv_data[..],
    )?;

    let config = AdvertisementParameters {
        timeout: Some(Duration::from_secs(1)),
        ..Default::default()
    };

    loop {
        info!("[adv] advertising");
        let mut advertiser = peripheral
            .advertise(
                &config,
                Advertisement::ConnectableScannableUndirected {
                    adv_data: &adv_data[..],
                    scan_data: &[],
                },
            )
            .await?;
        info!("[adv] waiting for connection");
        let conn = advertiser.accept().await?;
        info!("[adv] connection established");

        // Keep connection alive
        let mut tick: u8 = 0;
        while conn.is_connected() {
            Timer::after(Duration::from_secs(2)).await;
            tick = tick.wrapping_add(1);
            info!("[adv] notifying connection of tick {}", tick);
            let _ = server
                .notify(&server.battery_service.level, &conn, &tick)
                .await;
        }
    }
}
