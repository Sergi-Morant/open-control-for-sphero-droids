//! Entrada/salida Bluetooth LE con btleplug (CoreBluetooth en macOS, BlueZ en Linux, WinRT en Windows).
//! Solo mueve bytes: el protocolo vive en `droid.rs`.

use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager};
use futures::StreamExt;
use tokio::sync::{mpsc, OnceCell};
use uuid::Uuid;

use crate::droid::Outgoing;
use crate::kind::Kind;

// API v2 (R2-D2, R2-Q5, BB-9E)
const V2_API: Uuid = Uuid::from_u128(0x00010002_574F_4F20_5370_6865726F2121);
const V2_ANTIDOS: Uuid = Uuid::from_u128(0x00020005_574F_4F20_5370_6865726F2121);
const V2_SECRET: &[u8] = b"usetheforce...band";
// API v1 (BB-8)
const V1_ANTIDOS: Uuid = Uuid::from_u128(0x22BB746F_2BBD_7554_2D6F_726568705327);
const V1_TXPOWER: Uuid = Uuid::from_u128(0x22BB746F_2BB2_7554_2D6F_726568705327);
const V1_WAKE: Uuid = Uuid::from_u128(0x22BB746F_2BBF_7554_2D6F_726568705327);
const V1_COMMAND: Uuid = Uuid::from_u128(0x22BB746F_2BA1_7554_2D6F_726568705327);
const V1_RESPONSE: Uuid = Uuid::from_u128(0x22BB746F_2BA6_7554_2D6F_726568705327);
const V1_SECRET: &[u8] = b"011i3";

pub enum Event {
    ScanStarted,
    ScanFinished,
    BluetoothError(String),
    Found { id: String, name: String, kind: Kind, rssi: Option<i16> },
    Ready(String),
    Notification(String, Vec<u8>),
    Rssi(String, i16),
    Failed(String, String),
    Disconnected(String),
}

#[derive(Clone)]
struct Notifier {
    tx: Sender<Event>,
    ctx: egui::Context,
}

impl Notifier {
    fn send(&self, e: Event) {
        let _ = self.tx.send(e);
        self.ctx.request_repaint();
    }
}

type Shared = Arc<OnceCell<(Manager, Adapter)>>;

pub struct Backend {
    rt: tokio::runtime::Runtime,
    adapter: Shared,
    notify: Notifier,
}

impl Backend {
    pub fn new(ctx: egui::Context, tx: Sender<Event>) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("no se pudo crear el runtime de tokio");
        Backend { rt, adapter: Arc::new(OnceCell::new()), notify: Notifier { tx, ctx } }
    }

    pub fn start_scan(&self) {
        let (cell, n) = (self.adapter.clone(), self.notify.clone());
        self.rt.spawn(async move {
            n.send(Event::ScanStarted);
            if let Err(e) = scan(&cell, &n).await {
                n.send(Event::BluetoothError(format!("{e:#}")));
            }
            n.send(Event::ScanFinished);
        });
    }

    /// Abre la conexión en segundo plano y devuelve el canal por el que se envían los paquetes.
    pub fn connect(&self, id: String, kind: Kind) -> mpsc::UnboundedSender<Outgoing> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (cell, n) = (self.adapter.clone(), self.notify.clone());
        self.rt.spawn(async move {
            if let Err(e) = session(&cell, &id, kind, &n, rx).await {
                n.send(Event::Failed(id.clone(), format!("{e:#}")));
            }
            n.send(Event::Disconnected(id));
        });
        tx
    }
}

async fn adapter(cell: &Shared) -> Result<Adapter> {
    let (_, adapter) = cell
        .get_or_try_init(|| async {
            let manager = Manager::new().await?;
            let adapter = manager
                .adapters()
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("No se encontró ningún adaptador Bluetooth"))?;
            Ok::<_, anyhow::Error>((manager, adapter))
        })
        .await?;
    Ok(adapter.clone())
}

async fn scan(cell: &Shared, n: &Notifier) -> Result<()> {
    let adapter = adapter(cell).await?;
    adapter.start_scan(ScanFilter::default()).await?;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        for p in adapter.peripherals().await? {
            let Some(props) = p.properties().await? else { continue };
            let name = props.local_name.or(props.advertisement_name).unwrap_or_default();
            if let Some(kind) = Kind::from_ble_name(&name) {
                n.send(Event::Found { id: p.id().to_string(), name, kind, rssi: props.rssi });
            }
        }
    }
    adapter.stop_scan().await?;
    Ok(())
}

async fn session(
    cell: &Shared,
    id: &str,
    kind: Kind,
    n: &Notifier,
    mut rx: mpsc::UnboundedReceiver<Outgoing>,
) -> Result<()> {
    let adapter = adapter(cell).await?;
    let p = adapter
        .peripherals()
        .await?
        .into_iter()
        .find(|p| p.id().to_string() == id)
        .ok_or_else(|| anyhow!("El droide ya no está disponible; vuelve a buscar"))?;

    p.connect().await?;
    p.discover_services().await?;
    let chars = p.characteristics();
    let find = |u: Uuid| chars.iter().find(|c| c.uuid == u).cloned().ok_or_else(|| anyhow!("Falta la característica {u}"));

    let (command, response) = if kind.uses_v2() {
        let (api, anti) = (find(V2_API)?, find(V2_ANTIDOS)?);
        p.subscribe(&api).await?;
        p.write(&anti, V2_SECRET, WriteType::WithResponse).await?;
        (api.clone(), api)
    } else {
        let (anti, tx_power, wake) = (find(V1_ANTIDOS)?, find(V1_TXPOWER)?, find(V1_WAKE)?);
        let (command, response) = (find(V1_COMMAND)?, find(V1_RESPONSE)?);
        p.subscribe(&response).await?;
        p.write(&anti, V1_SECRET, WriteType::WithResponse).await?;
        p.write(&tx_power, &[0x07], WriteType::WithResponse).await?;
        p.write(&wake, &[0x01], WriteType::WithResponse).await?;
        (command, response)
    };

    let mut notifications = p.notifications().await?;
    // En Windows la señal solo se actualiza mientras hay un escaneo activo.
    #[cfg(windows)]
    let _ = adapter.start_scan(ScanFilter::default()).await;
    n.send(Event::Ready(id.to_string()));

    let write_type = if command.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE) {
        WriteType::WithoutResponse
    } else {
        WriteType::WithResponse
    };
    let mut rssi_timer = tokio::time::interval(Duration::from_millis(700));

    loop {
        tokio::select! {
            note = notifications.next() => match note {
                Some(v) if v.uuid == response.uuid => n.send(Event::Notification(id.to_string(), v.value)),
                Some(_) => {}
                None => break,
            },
            out = rx.recv() => match out {
                Some(Outgoing::Data(d)) => p.write(&command, &d, write_type).await?,
                Some(Outgoing::Disconnect) | None => {
                    let _ = p.disconnect().await;
                    break;
                }
            },
            _ = rssi_timer.tick() => {
                if let Ok(r) = p.read_rssi().await {
                    n.send(Event::Rssi(id.to_string(), r));
                }
            }
        }
    }
    Ok(())
}
