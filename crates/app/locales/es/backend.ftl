# backend: what the player, the downloads, the lyrics and the updates tell when they fail.

backend-playback-interrupted = La reproducción está interrumpida por otra sesión de audio.
backend-output-open-failed = No se pudo abrir la salida de audio: { $error }
backend-output-both-failed = No se pudo usar la salida elegida ({ $error }) ni la predeterminada ({ $fallback }). Revisá la conexión del dispositivo y volvé a elegir una salida.
backend-output-default-failed = No se pudo usar la salida predeterminada: { $error }. Revisá la configuración de sonido del sistema y volvé a elegir una salida.
backend-engine-unavailable = El motor de audio no está disponible: { $message }
backend-lyrics-start-failed = No se pudo iniciar el servicio de letras. Volvé a abrir Surco.
backend-artist-not-found = No encontramos a { $name } en el catálogo.
backend-playlist-empty = No pudimos traer ninguna de las canciones de esa playlist.
backend-update-title = Hay una versión nueva de Surco
backend-update-body = La { $version } está lista. Abrí Surco para actualizar.
backend-update-no-program = Esta versión no trae un programa para este sistema.
backend-update-no-sum = La versión no dice cómo comprobar su programa.
backend-update-mismatch = Lo que se bajó no es el programa que la versión publicó.
backend-update-no-location = No se sabe dónde está instalado el programa.
backend-update-replace-failed = No se pudo reemplazar el programa: { $error }
backend-no-audio = YouTube no ofreció audio para este tema.
backend-download-cut = La descarga se cortó antes de terminar.
backend-save-failed = No se pudo guardar en el disco: { $error }
backend-cover = Portada
backend-lyrics-limited = El servicio de letras alcanzó su límite. Esperá { $seconds } { $seconds ->
        [one] segundo
       *[other] segundos
    } y volvé a intentar.
backend-lyrics-youtube-failed = No pudimos cargar la letra desde YouTube Music. Volvé a intentar.
backend-lyrics-timeout = El servicio de letras tardó demasiado. Volvé a intentar.
backend-lyrics-unreachable = No pudimos conectar con el servicio de letras. Revisá la conexión y volvé a intentar.
backend-lyrics-unavailable = El servicio de letras no está disponible (HTTP { $status }). Volvé a intentar.
backend-lyrics-unreadable = El servicio de letras envió una respuesta que no pudimos leer. Volvé a intentar.
backend-translation-unreadable = No se pudo leer la traducción.
backend-pictures = Imágenes
backend-handoff-no-network = Este equipo no está en una red local. Conectalo al mismo wifi que el otro dispositivo.
backend-handoff-unreadable = Lo que llegó no es una cuenta de Surco.
backend-handoff-newer = Esa cuenta viene de un Surco más nuevo. Actualizá este para traerla.
backend-handoff-unreachable = No se pudo llegar al otro dispositivo. Revisá que los dos estén en la misma red y que el diálogo siga abierto allá.
backend-handoff-code = Código para pasar la cuenta
backend-handoff-no-port = No hay un puerto libre para pasar la cuenta. Cerrá otras ventanas de Surco y probá de nuevo.
backend-recognize-unsupported = Reconocer canciones todavía no está en el iPhone.
backend-handoff-stopped = Alguien probó demasiados códigos en la red, así que se dejó de compartir. Cambiá una opción o cerrá y abrí de nuevo para tener otro código.
