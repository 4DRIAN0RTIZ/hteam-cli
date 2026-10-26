# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_hteam_global_optspecs
    string join \n json b/board= family= h/help V/version
end

function __fish_hteam_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_hteam_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_hteam_using_subcommand
    set -l cmd (__fish_hteam_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c hteam -n "__fish_hteam_needs_command" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_needs_command" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_needs_command" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_needs_command" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_needs_command" -s V -l version -d 'Print version'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "login" -d 'Iniciar sesión con cookies'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "doctor" -d 'Diagnosticar configuración, sesión y acceso al board'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "dashboard" -d 'Resumen personal: working-on-it, recordatorios y objetivos semanales'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "board" -d 'Gestionar boards'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "lists" -d 'Listar listas del board'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "cards" -d 'Operaciones con cards'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "open" -d 'Ver cards abiertos'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "closed" -d 'Ver cards cerrados'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "working" -d 'Gestionar working on it'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "project" -d 'Gestionar proyectos'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "objectives" -d 'Objetivos semanales (scraping de SharePad, sólo lectura)'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "checkin" -d 'Registrar entrada del día (check in)'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "reminders" -d 'Ver y gestionar reminders'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "daily-work" -d 'Ver historial de trabajo diario del equipo'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "users" -d 'Buscar usuarios por nombre'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "interactive" -d 'Modo interactivo REPL'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "tui" -d 'Board visual tipo kanban (TUI)'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "theme" -d 'Gestionar el tema de color del TUI'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "mcp" -d 'Iniciar servidor MCP (Model Context Protocol) sobre stdio'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "update" -d 'Descargar e instalar la última versión publicada de hteam-cli'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "completions" -d 'Generar script de autocompletado para la shell indicada'
complete -c hteam -n "__fish_hteam_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand login" -s s -l session-id -d 'Session ID (cookie)' -r
complete -c hteam -n "__fish_hteam_using_subcommand login" -s c -l csrf-token -d 'CSRF Token (cookie)' -r
complete -c hteam -n "__fish_hteam_using_subcommand login" -s b -l board -d 'Board number por defecto' -r
complete -c hteam -n "__fish_hteam_using_subcommand login" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand login" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand login" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand doctor" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand doctor" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand doctor" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand doctor" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand dashboard" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand dashboard" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand dashboard" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand dashboard" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand board; and not __fish_seen_subcommand_from list switch current help" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand board; and not __fish_seen_subcommand_from list switch current help" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand board; and not __fish_seen_subcommand_from list switch current help" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand board; and not __fish_seen_subcommand_from list switch current help" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand board; and not __fish_seen_subcommand_from list switch current help" -f -a "list" -d 'Listar boards configurados'
complete -c hteam -n "__fish_hteam_using_subcommand board; and not __fish_seen_subcommand_from list switch current help" -f -a "switch" -d 'Cambiar board activo'
complete -c hteam -n "__fish_hteam_using_subcommand board; and not __fish_seen_subcommand_from list switch current help" -f -a "current" -d 'Mostrar board actual'
complete -c hteam -n "__fish_hteam_using_subcommand board; and not __fish_seen_subcommand_from list switch current help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from list" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from list" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from list" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from switch" -s n -l name -d 'Nombre del board (opcional)' -r
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from switch" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from switch" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from switch" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from switch" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from current" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from current" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from current" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from current" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from help" -f -a "list" -d 'Listar boards configurados'
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from help" -f -a "switch" -d 'Cambiar board activo'
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from help" -f -a "current" -d 'Mostrar board actual'
complete -c hteam -n "__fish_hteam_using_subcommand board; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand lists" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand lists" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand lists" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand lists" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "lists" -d 'Listar todas las listas'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "list" -d 'Ver cards de una lista'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "search" -d 'Buscar y filtrar cards del board (texto, responsable, prioridad, labels)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "detail" -d 'Ver detalle de un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "link" -d 'Copiar el link de un card al portapapeles'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "create" -d 'Crear un nuevo card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "move" -d 'Mover un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "close" -d 'Cerrar una task (POST close/ de la familia operation o project)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "reopen" -d 'Reabrir una task (POST reopen/ de la familia operation o project)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "dates" -d 'Editar fechas de inicio y/o vencimiento (board-card dates/, YYYY-MM-DD)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "time" -d 'Fijar estimado y/o registrar tiempo gastado (board-card times/)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "update-desc" -d 'Actualizar descripción'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "comment" -d 'Comentar en un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "comments" -d 'Ver comentarios de un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "labels" -d 'Ver labels disponibles de un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "update" -d 'Actualizar campos de un card (nombre, descripción, prioridad)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "remind" -d 'Crear un recordatorio para un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "follow-up" -d 'Ver el seguimiento agendado de un card, si tiene uno'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "follow-up-complete" -d 'Completar un seguimiento agendado'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "follow-up-cancel" -d 'Cancelar (eliminar) un seguimiento agendado'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and not __fish_seen_subcommand_from lists list search detail link create move close reopen dates time update-desc comment comments labels update remind follow-up follow-up-complete follow-up-cancel help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from lists" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from lists" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from lists" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from lists" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from list" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from list" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from list" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from search" -s q -l query -d 'Texto libre (nombre o descripción, case-insensitive)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from search" -l responsible -d 'Filtrar por responsable (username/email o id numérico)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from search" -l priority -d 'Filtrar por prioridad (1=alta, 2=media, 3=baja u otro valor del API)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from search" -l label -d 'Label requerida (repetible; AND entre labels)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from search" -l list-id -d 'Limitar a una lista del board' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from search" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from search" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from search" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from search" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from detail" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from detail" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from detail" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from detail" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from link" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from link" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from link" -l no-copy -d 'Solo imprimir el link, sin copiarlo al portapapeles'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from link" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from link" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from create" -s n -l name -d 'Nombre del card' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from create" -s l -l list-id -d 'ID de la lista donde crear el card' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from create" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from create" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from create" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from move" -s t -l to -d 'Lista destino' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from move" -s f -l from -d 'Lista origen (opcional)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from move" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from move" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from move" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from move" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from close" -l family -d 'Familia API: `operation` o `project`. Si se omite, se infiere del `title_url` de la task' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from close" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from close" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from close" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from reopen" -l family -d 'Familia API: `operation` o `project`. Si se omite, se infiere del `title_url` de la task' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from reopen" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from reopen" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from reopen" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from dates" -l start -d 'Fecha de inicio (YYYY-MM-DD)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from dates" -l due -d 'Fecha de vencimiento (YYYY-MM-DD)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from dates" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from dates" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from dates" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from dates" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from time" -l estimate -d 'Estimado (ej. 1h 30m)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from time" -l spent -d 'Tiempo gastado (ej. 45m; +30m suma al acumulado)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from time" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from time" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from time" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from time" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update-desc" -s d -l description -d 'Nueva descripción' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update-desc" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update-desc" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update-desc" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update-desc" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comment" -s t -l text -d 'Comentario' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comment" -s b -l board -d 'Board number (si no está configurado)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comment" -s d -l date -d 'Fecha del comentario, formato "YYYY-MM-DD HH:MM" en hora local (opcional; por defecto ahora)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comment" -l reply-to -d 'ID del comentario al que responder' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comment" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comment" -s f -l follow -d 'Marcar el comentario para seguimiento (recibir notificaciones de respuestas)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comment" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comment" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comments" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comments" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comments" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from comments" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from labels" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from labels" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from labels" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from labels" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from labels" -a "set" -d 'Reemplazar todas las labels asignadas'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from labels" -a "add" -d 'Agregar labels sin quitar las existentes'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from labels" -a "remove" -d 'Quitar labels indicadas'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from labels" -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update" -s n -l name -d 'Nuevo nombre' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update" -s d -l description -d 'Nueva descripción' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update" -s p -l priority -d 'Prioridad (1=alta, 2=media, 3=baja)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update" -s r -l responsible -d 'Responsable (username)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from update" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from remind" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from remind" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from remind" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from remind" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up-complete" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up-complete" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up-complete" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up-complete" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up-cancel" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up-cancel" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up-cancel" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from follow-up-cancel" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "lists" -d 'Listar todas las listas'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "list" -d 'Ver cards de una lista'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "search" -d 'Buscar y filtrar cards del board (texto, responsable, prioridad, labels)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "detail" -d 'Ver detalle de un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "link" -d 'Copiar el link de un card al portapapeles'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "create" -d 'Crear un nuevo card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "move" -d 'Mover un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "close" -d 'Cerrar una task (POST close/ de la familia operation o project)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "reopen" -d 'Reabrir una task (POST reopen/ de la familia operation o project)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "dates" -d 'Editar fechas de inicio y/o vencimiento (board-card dates/, YYYY-MM-DD)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "time" -d 'Fijar estimado y/o registrar tiempo gastado (board-card times/)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "update-desc" -d 'Actualizar descripción'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "comment" -d 'Comentar en un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "comments" -d 'Ver comentarios de un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "labels" -d 'Ver labels disponibles de un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "update" -d 'Actualizar campos de un card (nombre, descripción, prioridad)'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "remind" -d 'Crear un recordatorio para un card'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "follow-up" -d 'Ver el seguimiento agendado de un card, si tiene uno'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "follow-up-complete" -d 'Completar un seguimiento agendado'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "follow-up-cancel" -d 'Cancelar (eliminar) un seguimiento agendado'
complete -c hteam -n "__fish_hteam_using_subcommand cards; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand open" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand open" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand open" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand open" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand closed" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand closed" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand closed" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand closed" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand working; and not __fish_seen_subcommand_from list start stop help" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand working; and not __fish_seen_subcommand_from list start stop help" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand working; and not __fish_seen_subcommand_from list start stop help" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand working; and not __fish_seen_subcommand_from list start stop help" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand working; and not __fish_seen_subcommand_from list start stop help" -f -a "list" -d 'Ver cards en los que estoy trabajando'
complete -c hteam -n "__fish_hteam_using_subcommand working; and not __fish_seen_subcommand_from list start stop help" -f -a "start" -d 'Empezar a trabajar en un card'
complete -c hteam -n "__fish_hteam_using_subcommand working; and not __fish_seen_subcommand_from list start stop help" -f -a "stop" -d 'Dejar de trabajar en un card'
complete -c hteam -n "__fish_hteam_using_subcommand working; and not __fish_seen_subcommand_from list start stop help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from list" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from list" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from list" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from start" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from start" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from start" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from start" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from stop" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from stop" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from stop" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from stop" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from help" -f -a "list" -d 'Ver cards en los que estoy trabajando'
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from help" -f -a "start" -d 'Empezar a trabajar en un card'
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from help" -f -a "stop" -d 'Dejar de trabajar en un card'
complete -c hteam -n "__fish_hteam_using_subcommand working; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "list" -d 'Listar proyectos (por defecto, en estado "Execution")'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "milestones" -d 'Ver progreso de milestones de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "milestone-create" -d 'Crear un milestone en un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "milestone-edit" -d 'Editar un milestone de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "task-create" -d 'Crear una task de proyecto dentro de un milestone'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "task-edit" -d 'Editar milestone, dependencia o fecha de inicio de una task de proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "tasks" -d 'Ver tasks de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "status" -d 'Resumir progreso, fechas y tiempos de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "members-set" -d 'Reemplazar los miembros de un proyecto por IDs de usuario'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "task-tree" -d 'Ver el árbol de dependencias de tasks de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "link" -d 'Copiar el link de un proyecto al portapapeles'
complete -c hteam -n "__fish_hteam_using_subcommand project; and not __fish_seen_subcommand_from list milestones milestone-create milestone-edit task-create task-edit tasks status members-set task-tree link help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from list" -s s -l status -d 'Estado a filtrar: All, Planning, Execution (default), Finished, Cancelled' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from list" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from list" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from list" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestones" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestones" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestones" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestones" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-create" -l name -d 'Nombre del milestone' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-create" -l description -d 'Descripción (opcional)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-create" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-create" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-create" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-create" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-edit" -l name -d 'Nuevo nombre (opcional)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-edit" -l description -d 'Nueva descripción (opcional; cadena vacía la limpia)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-edit" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-edit" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-edit" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from milestone-edit" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -l name -d 'Nombre de la task' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -l description -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -l estimate -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -l start-date -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -l dependence -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -l responsible -d 'ID o username del responsable' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -l priority -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-create" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-edit" -l milestone -d 'Nuevo ID de milestone' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-edit" -l dependence -d 'Nuevo ID de task de la que depende' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-edit" -l start-date -d 'Nueva fecha de inicio (YYYY-MM-DD)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-edit" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-edit" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-edit" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-edit" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from tasks" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from tasks" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from tasks" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from tasks" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from status" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from status" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from status" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from status" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from members-set" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from members-set" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from members-set" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from members-set" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-tree" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-tree" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-tree" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from task-tree" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from link" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from link" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from link" -l no-copy -d 'Solo imprimir el link, sin copiarlo al portapapeles'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from link" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from link" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "list" -d 'Listar proyectos (por defecto, en estado "Execution")'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "milestones" -d 'Ver progreso de milestones de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "milestone-create" -d 'Crear un milestone en un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "milestone-edit" -d 'Editar un milestone de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "task-create" -d 'Crear una task de proyecto dentro de un milestone'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "task-edit" -d 'Editar milestone, dependencia o fecha de inicio de una task de proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "tasks" -d 'Ver tasks de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "status" -d 'Resumir progreso, fechas y tiempos de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "members-set" -d 'Reemplazar los miembros de un proyecto por IDs de usuario'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "task-tree" -d 'Ver el árbol de dependencias de tasks de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "link" -d 'Copiar el link de un proyecto al portapapeles'
complete -c hteam -n "__fish_hteam_using_subcommand project; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and not __fish_seen_subcommand_from show help" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and not __fish_seen_subcommand_from show help" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and not __fish_seen_subcommand_from show help" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and not __fish_seen_subcommand_from show help" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and not __fish_seen_subcommand_from show help" -f -a "show" -d 'Mostrar los objetivos semanales del notebook de SharePad'
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and not __fish_seen_subcommand_from show help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and __fish_seen_subcommand_from show" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and __fish_seen_subcommand_from show" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and __fish_seen_subcommand_from show" -l force -d 'Ignorar el cache local y volver a scrapear SharePad'
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and __fish_seen_subcommand_from show" -s v -l verbose -d 'Mostrar además el origen del resultado (cache/red) y hace cuánto se sincronizó'
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and __fish_seen_subcommand_from show" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and __fish_seen_subcommand_from show" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and __fish_seen_subcommand_from help" -f -a "show" -d 'Mostrar los objetivos semanales del notebook de SharePad'
complete -c hteam -n "__fish_hteam_using_subcommand objectives; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand checkin" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand checkin" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand checkin" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand checkin" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and not __fish_seen_subcommand_from cancel help" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and not __fish_seen_subcommand_from cancel help" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and not __fish_seen_subcommand_from cancel help" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and not __fish_seen_subcommand_from cancel help" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and not __fish_seen_subcommand_from cancel help" -f -a "cancel" -d 'Cancelar un reminder por su ID'
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and not __fish_seen_subcommand_from cancel help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and __fish_seen_subcommand_from cancel" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and __fish_seen_subcommand_from cancel" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and __fish_seen_subcommand_from cancel" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and __fish_seen_subcommand_from cancel" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and __fish_seen_subcommand_from help" -f -a "cancel" -d 'Cancelar un reminder por su ID'
complete -c hteam -n "__fish_hteam_using_subcommand reminders; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand daily-work" -s u -l user -d 'Filtrar por usuario (username)' -r
complete -c hteam -n "__fish_hteam_using_subcommand daily-work" -s t -l type -d 'Tipo de actividad: time, tracking, objectives' -r
complete -c hteam -n "__fish_hteam_using_subcommand daily-work" -s r -l range -d 'Rango de tiempo: today, this_week, this_month' -r
complete -c hteam -n "__fish_hteam_using_subcommand daily-work" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand daily-work" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand daily-work" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand daily-work" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand users" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand users" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand users" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand users" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand interactive" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand interactive" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand interactive" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand interactive" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand tui" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand tui" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand tui" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand tui" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and not __fish_seen_subcommand_from list set help" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand theme; and not __fish_seen_subcommand_from list set help" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand theme; and not __fish_seen_subcommand_from list set help" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and not __fish_seen_subcommand_from list set help" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and not __fish_seen_subcommand_from list set help" -f -a "list" -d 'Listar los presets embebidos y los temas personalizados, marcando el activo'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and not __fish_seen_subcommand_from list set help" -f -a "set" -d 'Cambiar el tema activo del TUI (preset embebido o personalizado)'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and not __fish_seen_subcommand_from list set help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from list" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from list" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from list" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from set" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from set" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from set" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from set" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from help" -f -a "list" -d 'Listar los presets embebidos y los temas personalizados, marcando el activo'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from help" -f -a "set" -d 'Cambiar el tema activo del TUI (preset embebido o personalizado)'
complete -c hteam -n "__fish_hteam_using_subcommand theme; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand mcp" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand mcp" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand mcp" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand mcp" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand update" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand update" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand update" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand update" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand completions" -s b -l board -d 'ID del board a usar (operación o proyecto según --family)' -r
complete -c hteam -n "__fish_hteam_using_subcommand completions" -l family -d 'Familia del tablero: operation (default) o project' -r
complete -c hteam -n "__fish_hteam_using_subcommand completions" -l json -d 'Mostrar salida en formato JSON'
complete -c hteam -n "__fish_hteam_using_subcommand completions" -s h -l help -d 'Print help'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "login" -d 'Iniciar sesión con cookies'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "doctor" -d 'Diagnosticar configuración, sesión y acceso al board'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "dashboard" -d 'Resumen personal: working-on-it, recordatorios y objetivos semanales'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "board" -d 'Gestionar boards'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "lists" -d 'Listar listas del board'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "cards" -d 'Operaciones con cards'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "open" -d 'Ver cards abiertos'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "closed" -d 'Ver cards cerrados'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "working" -d 'Gestionar working on it'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "project" -d 'Gestionar proyectos'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "objectives" -d 'Objetivos semanales (scraping de SharePad, sólo lectura)'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "checkin" -d 'Registrar entrada del día (check in)'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "reminders" -d 'Ver y gestionar reminders'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "daily-work" -d 'Ver historial de trabajo diario del equipo'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "users" -d 'Buscar usuarios por nombre'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "interactive" -d 'Modo interactivo REPL'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "tui" -d 'Board visual tipo kanban (TUI)'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "theme" -d 'Gestionar el tema de color del TUI'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "mcp" -d 'Iniciar servidor MCP (Model Context Protocol) sobre stdio'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "update" -d 'Descargar e instalar la última versión publicada de hteam-cli'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "completions" -d 'Generar script de autocompletado para la shell indicada'
complete -c hteam -n "__fish_hteam_using_subcommand help; and not __fish_seen_subcommand_from login doctor dashboard board lists cards open closed working project objectives checkin reminders daily-work users interactive tui theme mcp update completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from board" -f -a "list" -d 'Listar boards configurados'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from board" -f -a "switch" -d 'Cambiar board activo'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from board" -f -a "current" -d 'Mostrar board actual'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "lists" -d 'Listar todas las listas'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "list" -d 'Ver cards de una lista'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "search" -d 'Buscar y filtrar cards del board (texto, responsable, prioridad, labels)'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "detail" -d 'Ver detalle de un card'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "link" -d 'Copiar el link de un card al portapapeles'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "create" -d 'Crear un nuevo card'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "move" -d 'Mover un card'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "close" -d 'Cerrar una task (POST close/ de la familia operation o project)'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "reopen" -d 'Reabrir una task (POST reopen/ de la familia operation o project)'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "dates" -d 'Editar fechas de inicio y/o vencimiento (board-card dates/, YYYY-MM-DD)'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "time" -d 'Fijar estimado y/o registrar tiempo gastado (board-card times/)'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "update-desc" -d 'Actualizar descripción'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "comment" -d 'Comentar en un card'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "comments" -d 'Ver comentarios de un card'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "labels" -d 'Ver labels disponibles de un card'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "update" -d 'Actualizar campos de un card (nombre, descripción, prioridad)'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "remind" -d 'Crear un recordatorio para un card'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "follow-up" -d 'Ver el seguimiento agendado de un card, si tiene uno'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "follow-up-complete" -d 'Completar un seguimiento agendado'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from cards" -f -a "follow-up-cancel" -d 'Cancelar (eliminar) un seguimiento agendado'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from working" -f -a "list" -d 'Ver cards en los que estoy trabajando'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from working" -f -a "start" -d 'Empezar a trabajar en un card'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from working" -f -a "stop" -d 'Dejar de trabajar en un card'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "list" -d 'Listar proyectos (por defecto, en estado "Execution")'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "milestones" -d 'Ver progreso de milestones de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "milestone-create" -d 'Crear un milestone en un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "milestone-edit" -d 'Editar un milestone de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "task-create" -d 'Crear una task de proyecto dentro de un milestone'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "task-edit" -d 'Editar milestone, dependencia o fecha de inicio de una task de proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "tasks" -d 'Ver tasks de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "status" -d 'Resumir progreso, fechas y tiempos de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "members-set" -d 'Reemplazar los miembros de un proyecto por IDs de usuario'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "task-tree" -d 'Ver el árbol de dependencias de tasks de un proyecto'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from project" -f -a "link" -d 'Copiar el link de un proyecto al portapapeles'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from objectives" -f -a "show" -d 'Mostrar los objetivos semanales del notebook de SharePad'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from reminders" -f -a "cancel" -d 'Cancelar un reminder por su ID'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from theme" -f -a "list" -d 'Listar los presets embebidos y los temas personalizados, marcando el activo'
complete -c hteam -n "__fish_hteam_using_subcommand help; and __fish_seen_subcommand_from theme" -f -a "set" -d 'Cambiar el tema activo del TUI (preset embebido o personalizado)'
