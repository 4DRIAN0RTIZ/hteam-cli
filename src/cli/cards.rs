use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use tabled::{Table, settings::Style};

use crate::client::HteamClient;
use crate::config::Config;


#[derive(Subcommand, Debug)]
pub enum CardsCommands {
    /// Listar todas las listas
    Lists,
    /// Ver cards de una lista
    List(ListCardsArgs),
    /// Ver detalle de un card
    Detail(DetailArgs),
    /// Crear un nuevo card
    Create(CreateArgs),
    /// Mover un card
    Move(MoveArgs),
    /// Actualizar descripción
    UpdateDesc(UpdateDescArgs),
    /// Comentar en un card
    Comment(CommentArgs),
    /// Ver labels disponibles de un card
    Labels(LabelsArgs),
    /// Actualizar campos de un card (nombre, descripción, prioridad)
    Update(UpdateArgs),
    /// Crear un recordatorio para un card
    Remind(RemindArgs),
}

#[derive(Args, Debug)]
pub struct ListCardsArgs {
    /// ID de la lista
    pub list_id: u64,
}

#[derive(Args, Debug)]
pub struct DetailArgs {
    /// ID del card
    pub card_id: u64,
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    /// Nombre del card
    #[arg(short, long)]
    pub name: String,

    /// ID de la lista donde crear el card
    #[arg(short, long)]
    pub list_id: Option<u64>,
}

#[derive(Args, Debug)]
pub struct MoveArgs {
    /// ID del card
    pub card_id: u64,
    
    /// Lista destino
    #[arg(short, long)]
    pub to: u64,
    
    /// Lista origen (opcional)
    #[arg(short, long)]
    pub from: Option<u64>,
}

#[derive(Args, Debug)]
pub struct UpdateDescArgs {
    /// ID del card
    pub card_id: u64,
    
    /// Nueva descripción
    #[arg(short, long)]
    pub description: String,
}

#[derive(Args, Debug)]
pub struct CommentArgs {
    /// ID del card
    pub card_id: u64,
    
    /// Comentario
    #[arg(short, long)]
    pub text: String,
    
    /// Board number (si no está configurado)
    #[arg(short, long)]
    pub board: Option<u64>,
}

#[derive(Args, Debug)]
pub struct LabelsArgs {
    /// ID del card
    pub card_id: u64,
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// ID del card
    pub card_id: u64,

    /// Nuevo nombre
    #[arg(short, long)]
    pub name: Option<String>,

    /// Nueva descripción
    #[arg(short, long)]
    pub description: Option<String>,

    /// Prioridad (1=alta, 2=media, 3=baja)
    #[arg(short, long)]
    pub priority: Option<String>,

    /// Responsable (username)
    #[arg(short, long)]
    pub responsible: Option<String>,
}

#[derive(Args, Debug)]
pub struct RemindArgs {
    /// ID del card
    pub card_id: u64,
}

pub async fn execute(cmd: CardsCommands, board: Option<u64>, json: bool) -> Result<()> {
    match cmd {
        CardsCommands::Lists => lists(board, json).await,
        CardsCommands::List(args) => list_cards(args, board, json).await,
        CardsCommands::Detail(args) => card_detail(args, json).await,
        CardsCommands::Create(args) => create_card(args, json).await,
        CardsCommands::Move(args) => move_card(args, board, json).await,
        CardsCommands::UpdateDesc(args) => update_desc(args, json).await,
        CardsCommands::Comment(args) => post_comment(args, board, json).await,
        CardsCommands::Labels(args) => card_labels(args, board, json).await,
        CardsCommands::Update(args) => update_card(args, board, json).await,
        CardsCommands::Remind(args) => card_remind(args, json).await,
    }
}

pub async fn lists(board: Option<u64>, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    
    let lists = client.get_lists(board).await?;
    
    if json {
        println!("{}", serde_json::to_string_pretty(&lists)?);
        return Ok(());
    }
    
    if lists.is_empty() {
        println!("⚠️  No se encontraron listas.");
        return Ok(());
    }
    
    let mut table = Table::new(&lists);
    table.with(Style::rounded());
    
    println!("\n📋 Listas del board:\n");
    println!("{}", table);
    println!();
    
    Ok(())
}

async fn list_cards(args: ListCardsArgs, board: Option<u64>, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    
    let (cards, _) = client.get_cards(args.list_id, board).await?;
    
    if json {
        println!("{}", serde_json::to_string_pretty(&cards)?);
        return Ok(());
    }
    
    if cards.is_empty() {
        println!("⚠️  No se encontraron cards en la lista {}.", args.list_id);
        return Ok(());
    }
    
    let mut table = Table::new(&cards);
    table.with(Style::rounded());
    
    println!("\n🎴 Cards en lista {}:\n", args.list_id);
    println!("{}", table);
    println!();
    
    Ok(())
}

pub async fn open_cards(board: Option<u64>, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    
    let cards = client.get_open_cards(board).await?;
    
    if json {
        println!("{}", serde_json::to_string_pretty(&cards)?);
        return Ok(());
    }
    
    if cards.is_empty() {
        println!("⚠️  No hay cards abiertos.");
        return Ok(());
    }
    
    let mut table = Table::new(&cards);
    table.with(Style::rounded());
    
    println!("\n🎴 Cards abiertos:\n");
    println!("{}", table);
    println!();
    
    Ok(())
}

pub async fn closed_cards(board: Option<u64>, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    
    let cards = client.get_closed_cards(board).await?;
    
    if json {
        println!("{}", serde_json::to_string_pretty(&cards)?);
        return Ok(());
    }
    
    if cards.is_empty() {
        println!("⚠️  No hay cards cerrados.");
        return Ok(());
    }
    
    let mut table = Table::new(&cards);
    table.with(Style::rounded());
    
    println!("\n🎴 Cards cerrados:\n");
    println!("{}", table);
    println!();
    
    Ok(())
}

async fn card_detail(args: DetailArgs, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    
    let detail = client.get_card_detail(args.card_id).await?;
    
    if json {
        println!("{}", serde_json::to_string_pretty(&detail)?);
        return Ok(());
    }
    
    println!("\n🎴 Card #{}: {}", detail.id, detail.name.bold());
    if let Some(status) = &detail.status {
        println!("   Estado: {}", status);
    }
    
    if let Some(list) = detail.list {
        println!("   Lista: {} ({})", list.name, list.id);
    }
    
    if !detail.labels.is_empty() {
        let labels: Vec<String> = detail.labels.iter()
            .map(|l| l.name.clone())
            .collect();
        println!("   Labels: {}", labels.join(", "));
    }
    
    if let Some(desc) = detail.description {
        println!("\n   Descripción:\n   {}", desc);
    }
    
    println!();
    
    Ok(())
}

async fn create_card(args: CreateArgs, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    
    let card = client.create_card(&args.name, args.list_id, None).await?;
    
    if json {
        println!("{}", serde_json::to_string_pretty(&card)?);
        return Ok(());
    }
    
    println!("\n✅ Card creado exitosamente:");
    println!("   ID: {}", card.id);
    println!("   Nombre: {}", card.name);
    println!();
    
    Ok(())
}

async fn move_card(args: MoveArgs, board: Option<u64>, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    
    // If from_list is not provided, use default 1 (Open)
    let from_list = args.from.unwrap_or(1);
    
    client.move_card(args.card_id, from_list, args.to, board).await?;
    
    if json {
        println!("{{\"success\": true, \"card_id\": {}, \"from_list\": {}, \"to_list\": {}}}", 
                 args.card_id, from_list, args.to);
        return Ok(());
    }
    
    println!("\n✅ Card {} movido de lista {} a lista {}", args.card_id, from_list, args.to);
    println!();
    
    Ok(())
}

async fn update_desc(args: UpdateDescArgs, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;
    
    client.update_card_description(args.card_id, &args.description).await?;
    
    if json {
        println!("{{\"success\": true, \"card_id\": {}}}", args.card_id);
        return Ok(());
    }
    
    println!("\n✅ Descripción actualizada para card {}", args.card_id);
    println!();
    
    Ok(())
}

async fn post_comment(args: CommentArgs, board: Option<u64>, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;

    client.post_comment(args.card_id, &args.text, board).await?;

    if json {
        println!("{{\"success\": true, \"card_id\": {}}}", args.card_id);
        return Ok(());
    }

    println!("\n💬 Comentario agregado al card {}", args.card_id);
    println!();

    Ok(())
}

async fn card_labels(args: LabelsArgs, board: Option<u64>, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;

    let labels = client.get_card_labels(args.card_id, board).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&labels)?);
        return Ok(());
    }

    if labels.is_empty() {
        println!("⚠️  No se encontraron labels para el card {}.", args.card_id);
        return Ok(());
    }

    println!("\n🏷️  Labels del card {}:\n", args.card_id);
    for label in &labels {
        println!("   {} — {} ({})", label.id, label.name, label.color);
    }
    println!();

    Ok(())
}

async fn update_card(args: UpdateArgs, board: Option<u64>, json: bool) -> Result<()> {
    let config = Config::load()?;
    let board_number = board
        .or_else(|| config.get_board_number())
        .ok_or_else(|| anyhow::anyhow!("No hay board configurado. Usa --board o 'hteam board switch'."))?;

    let client = HteamClient::with_auth(config).await?;

    let detail = client.get_card_detail(args.card_id).await?;

    client.update_card_full(
        args.card_id,
        board_number,
        &detail,
        args.name.as_deref(),
        args.description.as_deref(),
        args.priority.as_deref(),
        args.responsible.as_deref(),
    ).await?;

    if json {
        println!("{{\"success\": true, \"card_id\": {}}}", args.card_id);
        return Ok(());
    }

    println!("\n✅ Card {} actualizado.", args.card_id);
    println!();

    Ok(())
}

async fn card_remind(args: RemindArgs, json: bool) -> Result<()> {
    let config = Config::load()?;
    let client = HteamClient::with_auth(config).await?;

    client.set_reminder(args.card_id).await?;

    if json {
        println!("{{\"success\": true, \"card_id\": {}}}", args.card_id);
        return Ok(());
    }

    println!("\n🔔 Recordatorio creado para el card {}.", args.card_id);
    println!();

    Ok(())
}
