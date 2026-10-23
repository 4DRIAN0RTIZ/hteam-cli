use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use tabled::{settings::Style, Table};

use crate::client::TaskFamily;
use crate::operations::{self, Session};

#[derive(Subcommand, Debug)]
pub enum CardsCommands {
    /// Listar todas las listas
    Lists,
    /// Ver cards de una lista
    List(ListCardsArgs),
    /// Buscar y filtrar cards del board (texto, responsable, prioridad, labels)
    Search(SearchArgs),
    /// Ver detalle de un card
    Detail(DetailArgs),
    /// Copiar el link de un card al portapapeles
    Link(LinkArgs),
    /// Crear un nuevo card
    Create(CreateArgs),
    /// Mover un card
    Move(MoveArgs),
    /// Cerrar una task (POST close/ de la familia operation o project)
    Close(LifecycleArgs),
    /// Reabrir una task (POST reopen/ de la familia operation o project)
    Reopen(LifecycleArgs),
    /// Editar fechas de inicio y/o vencimiento (board-card dates/, YYYY-MM-DD)
    Dates(DatesArgs),
    /// Fijar estimado y/o registrar tiempo gastado (board-card times/)
    Time(TimeArgs),
    /// Actualizar descripción
    UpdateDesc(UpdateDescArgs),
    /// Comentar en un card
    Comment(CommentArgs),
    /// Ver comentarios de un card
    Comments(CommentsArgs),
    /// Ver labels disponibles de un card
    Labels(LabelsArgs),
    /// Actualizar campos de un card (nombre, descripción, prioridad)
    Update(UpdateArgs),
    /// Crear un recordatorio para un card
    Remind(RemindArgs),
    /// Ver el seguimiento agendado de un card, si tiene uno
    FollowUp(FollowUpArgs),
    /// Completar un seguimiento agendado
    FollowUpComplete(FollowUpIdArgs),
    /// Cancelar (eliminar) un seguimiento agendado
    FollowUpCancel(FollowUpIdArgs),
}

#[derive(Args, Debug)]
pub struct ListCardsArgs {
    /// ID de la lista
    pub list_id: u64,
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Texto libre (nombre o descripción, case-insensitive)
    #[arg(short = 'q', long)]
    pub query: Option<String>,

    /// Filtrar por responsable (username/email o id numérico)
    #[arg(long)]
    pub responsible: Option<String>,

    /// Filtrar por prioridad (1=alta, 2=media, 3=baja u otro valor del API)
    #[arg(long)]
    pub priority: Option<String>,

    /// Label requerida (repetible; AND entre labels)
    #[arg(long = "label")]
    pub labels: Vec<String>,

    /// Limitar a una lista del board
    #[arg(long)]
    pub list_id: Option<u64>,
}

#[derive(Args, Debug)]
pub struct DetailArgs {
    /// ID del card
    pub card_id: u64,
}

#[derive(Args, Debug)]
pub struct LinkArgs {
    /// ID del card
    pub card_id: u64,

    /// Solo imprimir el link, sin copiarlo al portapapeles
    #[arg(long)]
    pub no_copy: bool,
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
pub struct LifecycleArgs {
    /// ID de la task / card
    pub card_id: u64,

    /// Familia API: `operation` o `project`. Si se omite, se infiere del `title_url` de la task.
    #[arg(long)]
    pub family: Option<String>,
}

#[derive(Args, Debug)]
pub struct DatesArgs {
    /// ID de la task / card
    pub card_id: u64,

    /// Fecha de inicio (YYYY-MM-DD)
    #[arg(long)]
    pub start: Option<String>,

    /// Fecha de vencimiento (YYYY-MM-DD)
    #[arg(long)]
    pub due: Option<String>,
}

#[derive(Args, Debug)]
pub struct TimeArgs {
    /// ID de la task / card
    pub card_id: u64,

    /// Estimado (ej. 1h 30m)
    #[arg(long)]
    pub estimate: Option<String>,

    /// Tiempo gastado (ej. 45m; +30m suma al acumulado)
    #[arg(long)]
    pub spent: Option<String>,
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

    /// Marcar el comentario para seguimiento (recibir notificaciones de respuestas)
    #[arg(short, long)]
    pub follow: bool,

    /// Fecha del comentario, formato "YYYY-MM-DD HH:MM" en hora local (opcional; por defecto ahora)
    #[arg(short, long)]
    pub date: Option<String>,

    /// ID del comentario al que responder
    #[arg(long)]
    pub reply_to: Option<u64>,
}

#[derive(Args, Debug)]
pub struct CommentsArgs {
    /// ID del card
    pub card_id: u64,
}

#[derive(Args, Debug)]
pub struct LabelsArgs {
    /// ID del card
    pub card_id: u64,

    #[command(subcommand)]
    pub command: Option<CardLabelsCommand>,
}

#[derive(Subcommand, Debug)]
pub enum CardLabelsCommand {
    /// Reemplazar todas las labels asignadas
    Set {
        #[arg(value_name = "LABEL_ID", num_args = 0..)]
        label_ids: Vec<u64>,
    },
    /// Agregar labels sin quitar las existentes
    Add { label_ids: Vec<u64> },
    /// Quitar labels indicadas
    Remove { label_ids: Vec<u64> },
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

#[derive(Args, Debug)]
pub struct FollowUpArgs {
    /// ID del card
    pub card_id: u64,
}

#[derive(Args, Debug)]
pub struct FollowUpIdArgs {
    /// ID del seguimiento (no el id del card ni el número visible — se obtiene con `hteam cards follow-up <card_id>`)
    pub follow_up_id: u64,
}

pub async fn execute(
    cmd: CardsCommands,
    board: Option<u64>,
    family: TaskFamily,
    json: bool,
) -> Result<()> {
    match cmd {
        CardsCommands::Lists => lists(board, family, json).await,
        CardsCommands::List(args) => list_cards(args, board, family, json).await,
        CardsCommands::Search(args) => search_cards(args, board, family, json).await,
        CardsCommands::Detail(args) => card_detail(args, json).await,
        CardsCommands::Link(args) => card_link(args, board, family, json).await,
        CardsCommands::Create(args) => create_card(args, json).await,
        CardsCommands::Move(args) => move_card(args, board, json).await,
        CardsCommands::Close(args) => close_card(args, json).await,
        CardsCommands::Reopen(args) => reopen_card(args, json).await,
        CardsCommands::Dates(args) => set_dates(args, board, family, json).await,
        CardsCommands::Time(args) => set_time(args, board, family, json).await,
        CardsCommands::UpdateDesc(args) => update_desc(args, json).await,
        CardsCommands::Comment(args) => post_comment(args, board, json).await,
        CardsCommands::Comments(args) => list_comments(args, json).await,
        CardsCommands::Labels(args) => card_labels(args, board, json).await,
        CardsCommands::Update(args) => update_card(args, board, json).await,
        CardsCommands::Remind(args) => card_remind(args, json).await,
        CardsCommands::FollowUp(args) => follow_up(args, board, json).await,
        CardsCommands::FollowUpComplete(args) => follow_up_complete(args, json).await,
        CardsCommands::FollowUpCancel(args) => follow_up_cancel(args, json).await,
    }
}

pub async fn lists(board: Option<u64>, family: TaskFamily, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let lists = operations::cards::list_lists_for(&session.client, board, family).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&lists)?);
        return Ok(());
    }

    if lists.is_empty() {
        println!("No se encontraron listas.");
        return Ok(());
    }

    let mut table = Table::new(&lists);
    table.with(Style::rounded());

    println!("\nListas del board:\n");
    println!("{}", table);
    println!();

    Ok(())
}

async fn list_cards(
    args: ListCardsArgs,
    board: Option<u64>,
    family: TaskFamily,
    json: bool,
) -> Result<()> {
    let session = Session::open().await?;

    let (cards, _) =
        operations::cards::list_cards_for(&session.client, args.list_id, board, family).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&cards)?);
        return Ok(());
    }

    if cards.is_empty() {
        println!("No se encontraron cards en la lista {}.", args.list_id);
        return Ok(());
    }

    let mut table = Table::new(&cards);
    table.with(Style::rounded());

    println!("\nCards en lista {}:\n", args.list_id);
    println!("{}", table);
    println!();

    Ok(())
}

async fn search_cards(
    args: SearchArgs,
    board: Option<u64>,
    family: TaskFamily,
    json: bool,
) -> Result<()> {
    let session = Session::open().await?;

    let filters = operations::cards::CardSearchFilters {
        text: args.query,
        responsible: args.responsible,
        priority: args.priority,
        labels: args.labels,
    };
    if filters.is_empty() && args.list_id.is_none() && !json {
        eprintln!(
            "{} Sin filtros: se listarán todas las cards del board (puede tardar).",
            "ℹ".blue()
        );
    }
    let scope = operations::cards::CardSearchScope {
        board,
        list_id: args.list_id,
        family,
    };

    let hits = operations::cards::search(&session.client, scope, filters).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&hits)?);
        return Ok(());
    }

    if hits.is_empty() {
        println!("Ninguna card coincide con los filtros.");
        return Ok(());
    }

    #[derive(tabled::Tabled)]
    struct Row {
        #[tabled(rename = "Lista")]
        list: String,
        #[tabled(rename = "List ID")]
        list_id: u64,
        #[tabled(rename = "ID")]
        id: u64,
        #[tabled(rename = "Nombre")]
        name: String,
        #[tabled(rename = "Responsable")]
        responsible: String,
        #[tabled(rename = "Labels")]
        labels: String,
    }

    let rows: Vec<Row> = hits
        .iter()
        .map(|h| Row {
            list: h.list_name.clone(),
            list_id: h.list_id,
            id: h.card.id,
            name: h.card.name.clone(),
            responsible: h
                .card
                .responsible
                .as_ref()
                .and_then(operations::cards::responsible_display)
                .unwrap_or_else(|| "-".into()),
            labels: h
                .card
                .labels
                .iter()
                .map(|l| l.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        })
        .collect();

    let mut table = Table::new(&rows);
    table.with(Style::rounded());

    println!("\n{} card(s):\n", rows.len());
    println!("{}", table);
    println!();

    Ok(())
}

pub async fn open_cards(board: Option<u64>, family: TaskFamily, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let cards = operations::cards::open_cards_for(&session.client, board, family).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&cards)?);
        return Ok(());
    }

    if cards.is_empty() {
        println!("No hay cards abiertos.");
        return Ok(());
    }

    let mut table = Table::new(&cards);
    table.with(Style::rounded());

    println!("\nCards abiertos:\n");
    println!("{}", table);
    println!();

    Ok(())
}

pub async fn closed_cards(board: Option<u64>, family: TaskFamily, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let cards = operations::cards::closed_cards_for(&session.client, board, family).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&cards)?);
        return Ok(());
    }

    if cards.is_empty() {
        println!("No hay cards cerrados.");
        return Ok(());
    }

    let mut table = Table::new(&cards);
    table.with(Style::rounded());

    println!("\nCards cerrados:\n");
    println!("{}", table);
    println!();

    Ok(())
}

async fn card_detail(args: DetailArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let detail = operations::cards::card_detail_enriched(&session.client, args.card_id).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&detail)?);
        return Ok(());
    }

    println!("\nCard #{}: {}", detail.id, detail.name.bold());
    if let Some(num) = detail.number {
        println!("   Número: #{num}");
    }
    if let Some(status) = &detail.status {
        println!("   Estado: {}", status);
    }
    if let Some(url) = detail.title_url.as_deref() {
        if let Some((family, process_id)) = TaskFamily::from_title_url_parts(url) {
            println!("   Familia: {} ({process_id})", family.as_str());
        }
    }
    if let Some(name) = detail.milestone_name.as_deref() {
        match detail.milestone {
            Some(id) => println!("   Milestone: {name} ({id})"),
            None => println!("   Milestone: {name}"),
        }
    } else if let Some(id) = detail.milestone {
        println!("   Milestone: {id}");
    }

    if let Some(list) = detail.list {
        println!("   Lista: {} ({})", list.name, list.id);
    }

    if !detail.labels.is_empty() {
        let labels: Vec<String> = detail.labels.iter().map(|l| l.name.clone()).collect();
        println!("   Labels: {}", labels.join(", "));
    }

    let estimate = detail.time_estimated.as_deref().unwrap_or("-");
    let spent = detail.time_spend.as_deref().unwrap_or("-");
    println!("   Tiempo: estimado {estimate} · gastado {spent}");

    if let Some(desc) = detail.description {
        println!("\n   Descripción:\n   {}", desc);
    }

    println!();

    Ok(())
}

async fn card_link(
    args: LinkArgs,
    board: Option<u64>,
    family: TaskFamily,
    json: bool,
) -> Result<()> {
    let session = Session::open().await?;

    let url = operations::cards::card_url_for(&session.client, args.card_id, board, family).await?;

    let copied = if args.no_copy {
        false
    } else {
        operations::clipboard::copy(&url).is_ok()
    };

    if json {
        println!(
            "{{\"card_id\": {}, \"url\": \"{}\", \"copied\": {}}}",
            args.card_id, url, copied
        );
        return Ok(());
    }

    println!("\n{} {}", operations::COPY_ICON, url);
    if copied {
        println!("   (copiado al portapapeles)");
    }
    println!();

    Ok(())
}

async fn create_card(args: CreateArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let card = operations::cards::create_card(&session.client, &args.name, args.list_id).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&card)?);
        return Ok(());
    }

    println!("\nCard creado exitosamente:");
    println!("   ID: {}", card.id);
    println!("   Nombre: {}", card.name);
    println!();

    Ok(())
}

async fn move_card(args: MoveArgs, board: Option<u64>, json: bool) -> Result<()> {
    let session = Session::open().await?;

    // If from_list is not provided, operations::cards::move_card defaults to 1 (Open)
    let from_list = args.from.unwrap_or(1);

    operations::cards::move_card(&session.client, args.card_id, args.from, args.to, board).await?;

    if json {
        println!(
            "{{\"success\": true, \"card_id\": {}, \"from_list\": {}, \"to_list\": {}}}",
            args.card_id, from_list, args.to
        );
        return Ok(());
    }

    println!(
        "\nCard {} movido de lista {} a lista {}",
        args.card_id, from_list, args.to
    );
    println!();

    Ok(())
}

async fn close_card(args: LifecycleArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;
    let result =
        operations::cards::close_card(&session.client, args.card_id, args.family.as_deref())
            .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!(
        "\nTask {} cerrada (familia {})\n   {}\n",
        result.card_id, result.family, result.status
    );
    Ok(())
}

async fn reopen_card(args: LifecycleArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;
    let result =
        operations::cards::reopen_card(&session.client, args.card_id, args.family.as_deref())
            .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!(
        "\nTask {} reabierta (familia {})\n   {}\n",
        result.card_id, result.family, result.status
    );
    Ok(())
}

async fn set_dates(
    args: DatesArgs,
    board: Option<u64>,
    family: TaskFamily,
    json: bool,
) -> Result<()> {
    let session = Session::open().await?;
    let result = operations::cards::set_dates_for(
        &session.client,
        args.card_id,
        board,
        family,
        args.start.as_deref(),
        args.due.as_deref(),
    )
    .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!(
        "\nFechas actualizadas para task {} (board_id {})",
        result.card_id, result.board_id
    );
    if let Some(d) = &result.start_date {
        println!("   start: {d}");
    }
    if let Some(d) = &result.due_date {
        println!("   due:   {d}");
    }
    println!();
    Ok(())
}

async fn set_time(
    args: TimeArgs,
    board: Option<u64>,
    family: TaskFamily,
    json: bool,
) -> Result<()> {
    let session = Session::open().await?;
    let result = operations::cards::set_times_for(
        &session.client,
        args.card_id,
        board,
        family,
        args.estimate.as_deref(),
        args.spent.as_deref(),
    )
    .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!(
        "\nTiempo actualizado para task {} (board_id {})",
        result.card_id, result.board_id
    );
    if let Some(value) = &result.estimate {
        println!("   estimado: {value}");
    }
    if let Some(value) = &result.spent {
        println!("   gastado:  {value}");
    }
    for warning in &result.warnings {
        println!("   {warning}");
    }
    println!();
    Ok(())
}

async fn update_desc(args: UpdateDescArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    operations::cards::update_description(&session.client, args.card_id, &args.description).await?;

    if json {
        println!("{{\"success\": true, \"card_id\": {}}}", args.card_id);
        return Ok(());
    }

    println!("\nDescripción actualizada para card {}", args.card_id);
    println!();

    Ok(())
}

async fn post_comment(args: CommentArgs, board: Option<u64>, json: bool) -> Result<()> {
    let session = Session::open().await?;

    operations::comments::post_comment(
        &session.client,
        args.card_id,
        &args.text,
        board,
        args.follow,
        args.date.as_deref(),
        args.reply_to,
    )
    .await?;

    if json {
        println!("{{\"success\": true, \"card_id\": {}}}", args.card_id);
        return Ok(());
    }

    println!("\nComentario agregado al card {}", args.card_id);
    println!();

    Ok(())
}

async fn list_comments(args: CommentsArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let comments = operations::comments::list_comments(&session.client, args.card_id).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&comments)?);
        return Ok(());
    }

    if comments.is_empty() {
        println!("No hay comentarios en el card {}.", args.card_id);
        return Ok(());
    }

    println!("\nComentarios del card {}:\n", args.card_id);
    let mut table = Table::new(&comments);
    table.with(Style::rounded());
    println!("{}", table);
    println!();

    Ok(())
}

async fn card_labels(args: LabelsArgs, board: Option<u64>, json: bool) -> Result<()> {
    let session = Session::open().await?;
    let Some(command) = args.command else {
        let labels = operations::cards::card_labels(&session.client, args.card_id, board).await?;
        if json {
            println!("{}", serde_json::to_string_pretty(&labels)?);
            return Ok(());
        }

        if labels.is_empty() {
            println!("No hay labels asignadas al card {}.", args.card_id);
            return Ok(());
        }

        println!("\n Labels del card {}:\n", args.card_id);
        for label in &labels {
            println!("   {} — {} ({})", label.id, label.name, label.color);
        }
        println!();
        return Ok(());
    };

    let (action, label_ids) = match command {
        CardLabelsCommand::Set { label_ids } => (
            "set",
            operations::cards::set_card_labels(&session.client, args.card_id, board, &label_ids)
                .await?,
        ),
        CardLabelsCommand::Add { label_ids } => (
            "add",
            operations::cards::add_card_labels(&session.client, args.card_id, board, &label_ids)
                .await?,
        ),
        CardLabelsCommand::Remove { label_ids } => (
            "remove",
            operations::cards::remove_card_labels(&session.client, args.card_id, board, &label_ids)
                .await?,
        ),
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "card_id": args.card_id,
                "action": action,
                "label_ids": label_ids,
            }))?
        );
    } else {
        println!(
            "Labels del card {} actualizadas ({action}): {:?}",
            args.card_id, label_ids
        );
    }

    Ok(())
}

async fn update_card(args: UpdateArgs, board: Option<u64>, json: bool) -> Result<()> {
    let session = Session::open().await?;
    let board_number =
        operations::resolve_board_number(&session.config, board).ok_or_else(|| {
            anyhow::anyhow!("No hay board configurado. Usa --board o 'hteam board switch'.")
        })?;

    operations::cards::update_card(
        &session.client,
        args.card_id,
        board_number,
        args.name.as_deref(),
        args.description.as_deref(),
        args.priority.as_deref(),
        args.responsible.as_deref(),
    )
    .await?;

    if json {
        println!("{{\"success\": true, \"card_id\": {}}}", args.card_id);
        return Ok(());
    }

    println!("\nCard {} actualizado.", args.card_id);
    println!();

    Ok(())
}

async fn card_remind(args: RemindArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    operations::reminders::create(&session.client, args.card_id).await?;

    if json {
        println!("{{\"success\": true, \"card_id\": {}}}", args.card_id);
        return Ok(());
    }

    println!("\nRecordatorio creado para el card {}.", args.card_id);
    println!();

    Ok(())
}

async fn follow_up(args: FollowUpArgs, board: Option<u64>, json: bool) -> Result<()> {
    let session = Session::open().await?;

    let follow_up =
        operations::comments::get_follow_up(&session.client, args.card_id, board).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&follow_up)?);
        return Ok(());
    }

    match follow_up {
        Some(f) => {
            println!(
                "\nSeguimiento #{} del card {}: {}",
                f.id, args.card_id, f.date
            );
            println!();
        }
        None => println!("El card {} no tiene un seguimiento agendado.", args.card_id),
    }

    Ok(())
}

async fn follow_up_complete(args: FollowUpIdArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    operations::comments::complete_follow_up(&session.client, args.follow_up_id).await?;

    if json {
        println!(
            "{{\"success\": true, \"follow_up_id\": {}}}",
            args.follow_up_id
        );
        return Ok(());
    }

    println!("\nSeguimiento {} completado.", args.follow_up_id);
    println!();

    Ok(())
}

async fn follow_up_cancel(args: FollowUpIdArgs, json: bool) -> Result<()> {
    let session = Session::open().await?;

    operations::comments::cancel_follow_up(&session.client, args.follow_up_id).await?;

    if json {
        println!(
            "{{\"success\": true, \"follow_up_id\": {}}}",
            args.follow_up_id
        );
        return Ok(());
    }

    println!("\n Seguimiento {} cancelado.", args.follow_up_id);
    println!();

    Ok(())
}
