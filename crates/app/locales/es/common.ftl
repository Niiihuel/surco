# Words several parts of the app share. Spanish is the app's first language: Rioplatense,
# with voseo ("querés", "podés").

common-artist = Artista
common-artists = Artistas
common-album = Álbum
common-albums = Álbumes
common-song = Canción
common-songs-heading = Canciones
common-top-result = Resultado más relevante
common-songs = { $count ->
    [one] 1 canción
   *[other] { $count } canciones
}
common-playlist = Playlist
common-cancel = Cancelar
common-close = Cerrar
common-delete = Eliminar
common-remove = Quitar
common-clear = Borrar
common-retry = Reintentar
common-play = Reproducir
common-pause = Pausar
common-options = Opciones
common-song-options = Opciones de la canción
common-share-song = Compartir canción
common-like = Me gusta
common-liked-songs = Tus me gusta
common-downloads = Descargas
common-settings = Configuración
common-equalizer = Ecualizador
common-library = Tu biblioteca
common-search = Buscar
common-search-placeholder = ¿Qué querés escuchar?
common-back = Volver
common-all = Todo
common-follow = Seguir
common-following = Siguiendo
common-link-copied = Enlace copiado
common-no-results = Sin resultados
common-next-song = Siguiente canción
common-create-playlist = Crear playlist
common-import = Importar
common-creator = Creador
common-code = Código
common-title = Título
common-minutes = { $minutes } min
common-hours-minutes = { $hours } h { $minutes } min
common-language = Idioma
common-language-detail = Las palabras de la app, en este idioma.
common-recommended = Recomendaciones
common-recommended-detail = Basado en el contenido de esta playlist
common-recommended-listening = Según lo que escuchás
common-recommended-add = Añadir
common-recommended-refresh = Volver a cargar
common-recommended-add-title = Agregar { $title } a la playlist
common-refresh = Actualizar
common-patches = Notas de versión
common-patches-detail = Lo que trajo cada versión de Surco
common-patches-current = Esta versión
common-patches-date = { $day }/{ $month }/{ $year }
common-patches-failed = No pudimos traer las notas de versión
common-watch-video = Ver el video
common-translate-lyrics = Traducir la letra
common-original-lyrics = Ver la letra original
common-welcome-title = Traé tu música. Te vamos a encontrar más.
common-welcome-detail = Tus playlists de Spotify, Apple Music o YouTube Music, en Surco en unos simples pasos.
common-welcome-import = IMPORTAR AHORA
common-settings-playback = Reproducción
common-settings-playback-about = Reproducción automática • Crossfade
common-settings-audio = Calidad de audio
common-settings-audio-about = Calidad • Normalizar • Ecualizador
common-settings-devices = Conexiones y dispositivos
common-settings-devices-about = Salida de audio • Bluetooth
common-settings-display = Pantalla e idioma
common-settings-display-about = Movimiento • Videos • Idioma
common-settings-library = Biblioteca
common-settings-library-about = Orden • Vista • Importar música • Scrobbling
common-settings-about = Acerca de Surco
common-settings-about-about = Versión { $version } • Notas de versión
common-settings-storage = Almacenamiento y privacidad
common-settings-storage-about = Descargas • Recientes
common-settings-transitions = Transiciones entre canciones
common-settings-controls = Controles de reproducción
common-settings-streaming = Calidad de transmisión
common-settings-sound = Sonido
common-settings-your-library = Tu biblioteca
common-settings-bring = Traer música
common-clear-recent-searches = Eliminar búsquedas recientes

## First run
common-onboarding-welcome = Te damos la bienvenida a Surco
common-onboarding-welcome-detail = Tu música, en tu equipo. Sin servidores ni cuentas en la nube.
common-onboarding-fresh = Empezar de cero
common-onboarding-bring = Ya tengo Surco en otro dispositivo
common-onboarding-name = ¿Cómo querés que te llamemos?
common-onboarding-name-detail = Así aparece en tu perfil. Lo podés cambiar cuando quieras.
common-onboarding-name-placeholder = Tu nombre
common-onboarding-genres = ¿Qué te gusta escuchar?
common-onboarding-genres-detail = Elegí los géneros que quieras. Nos ayudan a mostrarte artistas.
common-onboarding-artists = Elegí 3 o más artistas que te gusten.
common-onboarding-artists-detail = Con ellos armamos tu inicio. Al elegir uno aparecen otros parecidos.
common-onboarding-search = Buscá artistas
common-onboarding-left = { $count ->
    [one] Te falta 1
   *[other] Te faltan { $count }
}
common-onboarding-chosen = { $count ->
    [one] Elegiste 1
   *[other] Elegiste { $count }
}
common-onboarding-next = Siguiente
common-onboarding-ready = Listo
common-onboarding-skip = Omitir
common-onboarding-done = ¡Todo listo, { $name }!
common-onboarding-done-detail = Armamos tu inicio con lo que elegiste.
common-onboarding-home = Ir al inicio
common-onboarding-step = Ir al paso { $step }
common-onboarding-taste = Volver a elegir artistas
common-onboarding-taste-detail = Los géneros y artistas con los que se arma tu inicio.
common-scan-code = Escanear código
common-onboarding-bring-title = Traé tu cuenta
common-onboarding-bring-detail = En el otro dispositivo, abrí Configuración › Pasar a otro dispositivo › Enviar.
common-onboarding-bring-type = O escribí la dirección que muestra:

## SponsorBlock
common-sponsorblock-title = Saltar lo que no es música
common-sponsorblock-detail = En los videos, las partes que la gente marcó en SponsorBlock.
common-sponsorblock-footer = Los segmentos vienen de sponsor.ajay.app (CC BY-NC-SA 4.0). Solo se envía un pedazo del hash del video, no cuál suena.
common-sponsorblock-music-offtopic = Partes que no son música
common-sponsorblock-sponsor = Patrocinios
common-sponsorblock-selfpromo = Autopromoción
common-sponsorblock-interaction = Pedidos de like y suscripción
common-sponsorblock-intro = Intros
common-sponsorblock-outro = Cierres y créditos
common-sponsorblock-preview = Adelantos y resúmenes
common-sponsorblock-filler = Relleno y chistes
common-sponsorblock-segment = Una parte
common-sponsorblock-skipped = Se saltó: { $what }
common-skip-silence = Saltar silencios
common-skip-silence-detail = Pasa de largo el silencio con que empieza y termina cada canción.

## Replay
common-replay = Tu Replay
common-replay-see = Ver tu Replay
common-replay-month = Este mes
common-replay-year = Este año
common-replay-all = Desde siempre
common-replay-empty = Todavía no hay nada para contar
common-replay-empty-detail = Escuchá un rato: una canción cuenta a los 30 segundos.
common-replay-minutes = { $count ->
    [one] minuto escuchado
   *[other] minutos escuchados
}
common-replay-plays = { $count ->
    [one] reproducción
   *[other] reproducciones
}
common-replay-songs = { $count ->
    [one] canción
   *[other] canciones
}
common-replay-artists = { $count ->
    [one] artista
   *[other] artistas
}
common-replay-plays-of = { $count ->
    [one] 1 reproducción
   *[other] { $count } reproducciones
}
common-replay-song-heard = { $plays } × · { $minutes } min
common-replay-top-songs = Lo que más escuchaste
common-replay-top-artists = Tus artistas
common-replay-when = Cuándo escuchás
common-replay-night = Sos de la noche: tu música se enciende cuando oscurece.
common-replay-morning = Sos de la mañana: arrancás el día con música.
common-replay-day = Te acompaña todo el día.
common-replay-evening = Las tardes son tu momento de darle play.
common-replay-streak = días seguidos con música
common-replay-busiest = Tu día con más música · { $minutes } min
common-replay-on-repeat = En repetición · { $times } veces en un mismo día
common-replay-days = { $count ->
    [one] { $count } día
   *[other] { $count } días
}
common-replay-hours = { $count ->
    [one] Más de una hora
   *[other] Más de { $count } horas
}

## Scrobbling
common-scrobble = Scrobbling
common-scrobble-footer = Lo que escuchás se envía a tu cuenta cuando escuchaste la mitad de la canción o cuatro minutos. Las claves quedan solo en este dispositivo y no viajan al pasar tu cuenta.
common-scrobble-listenbrainz-detail = Pegá tu token de listenbrainz.org/settings.
common-scrobble-lastfm-detail = Con tu propia clave de API de last.fm/api. La contraseña se usa una vez y no se guarda.
common-scrobble-token = Token de usuario
common-scrobble-key = Clave de API
common-scrobble-secret = Secreto compartido
common-scrobble-user = Usuario
common-scrobble-password = Contraseña
common-scrobble-connect = Conectar
common-scrobble-connecting = Conectando…
common-scrobble-sign-out = Desconectar
common-scrobble-signed = Conectado como { $user }
common-scrobble-signed-waiting = Conectado como { $user } · { $count ->
    [one] 1 escucha por enviar
   *[other] { $count } escuchas por enviar
}
common-scrobble-failed = No se pudo conectar

## Recognize
common-recognize-title = ¿Qué canción es esta?
common-recognize-offer = Escucha lo que suena cerca y te dice qué es.
common-recognize-detail = Escucha por el micrófono y le pregunta a Shazam. Solo se envía una huella del sonido, nunca la grabación.
common-recognize-start = Escuchar
common-recognize-stop = Dejar de escuchar
common-recognize-idle = Tocá para escuchar
common-recognize-listening = Escuchando… { $seconds } s
common-recognize-found = ¡La encontramos!
common-recognize-unknown = No la reconocimos. Probá más cerca de la música.
common-recognize-failed = No se pudo escuchar
common-recognize-search = Buscar en Surco
common-recognize-lately = Encontradas hace poco

## Together
common-together = Escuchar juntos
common-together-detail = Una sala con código: lo que suena en el anfitrión suena para todos, a la vez, cada uno en su dispositivo.
common-together-host = Crear una sala
common-together-host-detail = Sos el anfitrión: lo que pongas acá, lo escuchan todos los que entren con el código.
common-together-host-go = Crear sala
common-together-join = Unirme a una sala
common-together-join-detail = Escribí el código que te pasaron.
common-together-join-go = Unirme
common-together-bad-code = Ese no es el código de una sala. Son doce caracteres, como K7QX-M2PA-9DTE.
common-together-failed = No se pudo entrar a la sala.
common-together-connecting = Conectando…
common-together-waiting = Esperando al anfitrión…
common-together-hosting = Sos el anfitrión · { $relays } relays
common-together-live = Escuchando con el anfitrión · { $relays } relays
common-together-lost = No sabemos nada del anfitrión hace un rato…
common-together-ended = La sala terminó.
common-together-copy = Copiar código
common-together-open = Los invitados también controlan
common-together-open-detail = Pueden poner, pausar, adelantar y saltar canciones, no solo sugerirlas.
common-together-close = Cerrar la sala
common-together-leave = Salir de la sala
common-together-members = En la sala
common-together-host-tag = anfitrión
common-together-chat = Charla
common-together-chat-empty = Todavía nadie dijo nada.
common-together-chat-placeholder = Escribí algo
common-together-send = Enviar
common-together-joined = { $name } entró a la sala.
common-together-suggested = { $name } sugirió «{ $title }».
common-together-guest-suggest = Lo que elijas se le sugiere al anfitrión para su cola.
common-together-guest-open = El anfitrión te deja poner, pausar y saltar canciones para todos.
