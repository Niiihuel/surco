# Surco en tu iPhone con TestFlight

La app nueva en App Store Connect es **Surco Music**, Apple ID **6823059312**,
con Bundle ID `io.github.niiihuel.surco`. En el iPhone se muestra como **Surco**.
El registro anterior de dnmusic queda separado.
La membresía Apple Developer paga ya permite preparar esta distribución.
La compilación, firma y subida se realizan en GitHub Actions; los pasos de
certificado de abajo también funcionan desde Linux, sin Xcode local.

## Beta entregada · 0.3.0 (4.1) · 15 de octubre de 2026

La [ejecución 38746396955](https://github.com/Niiihuel/surco/actions/runs/38746396955)
terminó correctamente desde `main` en `7e76a02a7ffd3552246ce6d3b13399c4b12b1373`,
después del merge de [PR #11](https://github.com/Niiihuel/surco/pull/11). La
validación y subida de la ejecución número **4**, intento **1**, están completas.
El IPA descargado confirma **0.3.0 (4.1)**, Bundle ID correcto, SDK iOS 26.5,
mínimo iOS 15, audio en segundo plano, LaunchScreen compilado, firma/perfil
incorporados y entitlements de distribución. Su SHA-256 es
`42b5a8d85c7d6c415d75630709173477099a380aa65e48e11b80818f63546375`.
El tag anotado [`ios-v0.3.0-build4.1`](https://github.com/Niiihuel/surco/tree/ios-v0.3.0-build4.1)
ya está publicado sobre ese mismo commit.

Apple mostró **Procesando** a las 11:03 ART y completó el procesamiento. A las
**14:08 UTC (11:08 ART)** quedó verificada **0.3.0 (4.1)** como **En pruebas**
para el grupo existente **Surco interno**, con tres testers y una vigencia de
90 días. Se guardaron las notas de prueba en español y las respuestas de cifrado
estándar adicional al SO/distribución fuera de Francia. El build UUID es
`5f142d11-cc15-4b39-a55e-cbbf81cb91a3` y su ficha está en
[App Store Connect](https://appstoreconnect.apple.com/teams/1c3fae91-928d-4369-94f8-4a7ada6471b9/apps/6823059312/testflight/ios/5f142d11-cc15-4b39-a55e-cbbf81cb91a3).

Para actualizar: abrí **TestFlight → Surco → Actualizar** y comprobá que indique
**0.3.0 (4.1)**. La asignación se hizo al grupo interno existente; no se agregaron
testers ni se enviaron invitaciones nuevas. La instalación y la prueba física
de esta versión todavía no están confirmadas por el usuario.

El árbol del commit de entrega `7e76a02` coincide con `ef6dbbb`, que pasó
[CI de escritorio](https://github.com/Niiihuel/surco/actions/runs/38744477519) y
[CI iOS](https://github.com/Niiihuel/surco/actions/runs/38744477042), con captura
final aprobada y smoke de arranque de ocho segundos sin crash. El posterior
[CI de escritorio de main](https://github.com/Niiihuel/surco/actions/runs/38746377744)
también pasó en las tres plataformas. El
[CI iOS de main 38746377853](https://github.com/Niiihuel/surco/actions/runs/38746377853)
compiló bien, pero su intento 1 falló en `simctl install` por timeout de 120 segundos,
antes de arrancar la app, tras un boot de unos 88 segundos con DataMigration;
device-unsigned sí pasó. El único reintento del job fallido también agotó el
tiempo de instalación antes de arrancar la app, sin cambios de código ni de
timeout. El [PR #13](https://github.com/Niiihuel/surco/pull/13) alinea el
toolchain de CI unsigned con el usado para firmar esta beta. Su primer head
`65561cf` falló en readiness antes de instalar. En `940a4b1`,
[escritorio](https://github.com/Niiihuel/surco/actions/runs/38752153515) y el
[job device-unsigned](https://github.com/Niiihuel/surco/actions/runs/38752153497)
pasaron. El simulador completó readiness, instalación, lanzamiento y el primer
control de proceso tras ocho segundos; produjo un PNG válido, pero `simctl`
screenshot venció su timeout. No se ejecutaron el control final ni la escritura
de `smoke.json`: ese intento falló y su causa exacta no quedó establecida. El nuevo head
`5420763` amplía solo el presupuesto de screenshot a 180 segundos, conservando
retorno exitoso, ambos controles de proceso y `smoke.json`; 14 tests Python pasan.
En el [CI iOS final](https://github.com/Niiihuel/surco/actions/runs/38755079773)
pasaron simulador y device-unsigned; el
[CI de escritorio](https://github.com/Niiihuel/surco/actions/runs/38755079488)
pasó en las tres plataformas. El resultado se verificó a las 15:05:28 UTC:
retorno correcto de captura, ambos controles de proceso y smoke completo. El
PR #13 fue integrado como `50b692d11236c14aaea0755b7e23f0aa51610235` a las
15:08:59 UTC. La PNG nativa de 1206×2622, ambos controles de proceso, el smoke
de ocho segundos y la lista vacía de crash reports fueron revisados y aceptados. Esto no reemplaza la prueba física del iPhone. El fallo y ambos intentos se conservan en la
[validación de motion](ios-polish/MOTION-VALIDATION.md), junto con la procedencia
separada de capturas y clips. Las pruebas físicas de 0.3.0 siguen pendientes.

## Beta entregada · 0.2.0 (3.1) · 15 de octubre de 2026

**Surco 0.2.0 (3.1)** terminó de procesarse y figura **En pruebas** en
**Surco interno**, el grupo existente de tres testers. La información de cifrado
está guardada para distribución fuera de Francia y las notas de prueba incluyen
audio, dispositivos, teclado, gestos y Copy/Paste entre apps.

La compilación histórica **0.2.0 (3.1)** está en
[App Store Connect](https://appstoreconnect.apple.com/teams/1c3fae91-928d-4369-94f8-4a7ada6471b9/apps/6823059312/testflight/ios/c483fa6b-e4d2-40c5-b697-7e2cd8be7e3f).

La [ejecución firmada 38719103389](https://github.com/Niiihuel/surco/actions/runs/38719103389)
validó y subió el IPA sin errores desde
`e5c0e8df3a2a2ee356f104d261b4f97aca77c54b`, conservado en el tag
[`ios-v0.2.0-build3.1`](https://github.com/Niiihuel/surco/tree/ios-v0.2.0-build3.1).
Ese mismo commit pasó [CI de escritorio](https://github.com/Niiihuel/surco/actions/runs/38719090300)
y [CI de simulador y compilación para iPhone](https://github.com/Niiihuel/surco/actions/runs/38719090310).
El IPA descargado confirma versión/build, SDK 26.5, mínimo iOS 15, firma,
perfil incorporado y modo de audio en segundo plano. Las claves temporales del
runner se limpiaron al terminar.

Esta entrega conserva el motor y la UI Rust/limn, con las integraciones nativas
de media, teclado, rutas y portapapeles. Las pruebas automatizadas y visuales
están completas. El 15 de octubre de 2026 el usuario confirmó que **0.2.0 (3.1)**
reproduce correctamente en segundo plano y con el iPhone bloqueado. Quedan
pendientes las demás comprobaciones del [iPhone real](ios.md#build-and-validation):
Bluetooth/AirPlay, llamadas/interrupciones, comandos de auriculares, Dynamic
Island, teclado/gestos y Copy/Paste entre apps. Esa confirmación física corresponde
a esta beta; no acredita todavía el comportamiento de 0.3.0.

## Herramientas de distribución

Desde el **28 de abril de 2026**, Apple exige Xcode 26 o posterior y SDK iOS 26
o posterior para subir builds a App Store Connect, incluido TestFlight.
El workflow `TestFlight` usa `macos-26` ARM64 y
`/Applications/Xcode_26.6.app/Contents/Developer`, con SDK iOS 26.5, verificados
en el inventario oficial del runner. La app conserva iOS 15 como versión mínima
de ejecución: la versión del SDK de compilación es una decisión distinta.
El workflow `iOS` de pruebas unsigned usa su propia herramienta de simulador.
([Requisitos Apple](https://developer.apple.com/news/upcoming-requirements/?id=04282026a),
[imagen GitHub](https://github.com/actions/runner-images/blob/main/images/macos/macos-26-arm64-Readme.md))

Se fija Xcode 26.x porque el host actual usa el ciclo de aplicación UIKit de
winit. Enlazar con SDK 27 exige migrar a UIScene; esa migración precede a subir
el SDK a 27. ([Apple UIKit](https://developer.apple.com/documentation/uikit/transitioning-to-the-uikit-scene-based-life-cycle))

## 1. Registro de la app nueva

El registro [Surco Music](https://appstoreconnect.apple.com/apps/6823059312/testflight/ios)
ya existe para iOS, con el App ID explícito `io.github.niiihuel.surco`. El SKU es
un identificador interno de App Store Connect y no cambia el Bundle ID del build.
Aceptá los acuerdos pendientes de tu cuenta si Apple los muestra.

Los siete secrets de GitHub están cargados y el perfil App Store Connect fue
validado. El grupo **Surco interno** está creado, con distribución automática
desactivada; el build se asigna manualmente cuando Apple termina de procesarlo.

El preflight consulta la API real de App Store Connect y falla antes de compilar
si ese registro no existe o la clave no tiene acceso. No crea ni reutiliza una
app automáticamente. ([Alta oficial](https://developer.apple.com/help/app-store-connect/create-an-app-record/add-a-new-app/))

## 2. Certificado de distribución desde Linux

La configuración actual usa un certificado **iOS Distribution** válido porque
el cupo de **Apple Distribution** estaba completo. El script admite tanto la
identidad `iPhone Distribution` de ese certificado como `Apple Distribution`.
El perfil de Surco Music debe incluir exactamente el certificado del `.p12`.
Los pasos siguientes sirven para emitir o renovar material de firma desde Linux.

Guardá las claves fuera del repositorio. Estos comandos piden las contraseñas
de forma interactiva; no las incluyas como argumentos ni las pegues en el chat.

```bash
mkdir -p -m 700 ~/.local/share/surco-signing
cd ~/.local/share/surco-signing
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -aes-256-cbc -out distribution.key
openssl req -new -key distribution.key -out Surco.certSigningRequest
```

Completá tu nombre y email al crear el CSR. En el portal Apple, **Certificates →
+**, elegí el tipo de distribución disponible para iOS (`iOS Distribution` en
esta configuración), cargá `Surco.certSigningRequest` y descargá el `.cer` como
`distribution.cer`. Convertí y exportá el certificado junto con su clave:

```bash
openssl x509 -inform DER -in distribution.cer -out distribution.pem
openssl pkcs12 -export -inkey distribution.key -in distribution.pem -name "Surco Distribution" -out distribution.p12
```

La última contraseña protege el `.p12`; usala en el secret
`IOS_DISTRIBUTION_P12_PASSWORD`. El `.cer` solo no alcanza para firmar: el `.p12`
debe incluir la clave privada creada junto con ese CSR.

En **Profiles → + → Distribution → App Store Connect**, elegí el App ID de Surco
y ese mismo certificado de distribución. Descargá el perfil como
`Surco.mobileprovision`. Un perfil Development o Ad Hoc no sirve para esta
entrega. ([Perfil oficial](https://developer.apple.com/help/account/provisioning-profiles/create-an-app-store-provisioning-profile/))

## 3. Crear la clave de App Store Connect y cargar los secrets

En App Store Connect, **Users and Access → Integrations → App Store Connect API →
Team Keys**, creá una clave con rol Developer. Las claves de equipo tienen acceso
a todas las apps del equipo según su rol; no se pueden limitar solo a Surco.
Developer permite subir builds y trabajar con TestFlight, sin permiso para
publicar una versión en el App Store. Copiá su Key ID y el Issuer ID, y descargá
`AuthKey_<KEY_ID>.p8` en la misma carpeta privada.
La clave de API permite autenticar la consulta y la subida; el `.p12` firma la
app. ([API keys y alcance](https://developer.apple.com/help/app-store-connect/get-started/app-store-connect-api/))

El workflow usa estos siete secrets, ya presentes en la configuración inicial.
Para reponerlos, abrí **Niiihuel/surco → Settings → Secrets and variables → Actions
→ Repository secrets**:

- `IOS_DISTRIBUTION_P12_BASE64`: contenido Base64 de `distribution.p12`.
- `IOS_DISTRIBUTION_P12_PASSWORD`: contraseña que elegiste al exportar el `.p12`.
- `IOS_APP_STORE_PROFILE_BASE64`: contenido Base64 de `Surco.mobileprovision`.
- `APPLE_TEAM_ID`: Team ID de 10 caracteres de tu cuenta Apple Developer.
- `APP_STORE_CONNECT_KEY_ID`: Key ID de la clave API de equipo.
- `APP_STORE_CONNECT_ISSUER_ID`: Issuer ID UUID de esa clave.
- `APP_STORE_CONNECT_PRIVATE_KEY_BASE64`: contenido Base64 del archivo `.p8`.

Con GitHub CLI autenticado, los archivos se pueden cargar desde Linux sin
imprimir su contenido en la terminal:

```bash
base64 -w 0 distribution.p12 | gh secret set IOS_DISTRIBUTION_P12_BASE64 --repo Niiihuel/surco
base64 -w 0 Surco.mobileprovision | gh secret set IOS_APP_STORE_PROFILE_BASE64 --repo Niiihuel/surco
base64 -w 0 AuthKey_TU_KEY_ID.p8 | gh secret set APP_STORE_CONNECT_PRIVATE_KEY_BASE64 --repo Niiihuel/surco
gh secret set IOS_DISTRIBUTION_P12_PASSWORD --repo Niiihuel/surco
gh secret set APPLE_TEAM_ID --repo Niiihuel/surco
gh secret set APP_STORE_CONNECT_KEY_ID --repo Niiihuel/surco
gh secret set APP_STORE_CONNECT_ISSUER_ID --repo Niiihuel/surco
```

Los cuatro últimos comandos piden el valor de forma interactiva. Sustituí
`TU_KEY_ID` por el nombre real del archivo descargado. GitHub trata estos valores
como secretos. ([Guía de firma de GitHub](https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications))

## 4. Generar y subir el build

El workflow **TestFlight** ya está registrado y se ejecuta manualmente. La
[primera ejecución de firma y subida](https://github.com/Niiihuel/surco/actions/runs/38699781352)
terminó correctamente: generó el primer IPA firmado y lo subió a App Store Connect.
El build inicial 0.1.4 (1.1) terminó de procesarse, fue asignado a **Surco interno**
y el usuario confirmó su instalación y arranque en su iPhone.
Para generar otro IPA sin subirlo desde la rama revisada:

```bash
gh workflow run testflight.yml --repo Niiihuel/surco --ref main -f upload=false
```

Para solicitar la subida, se usa `-f upload=true`. GitHub permite ejecutar un
workflow registrado contra una rama o tag por API o CLI; el botón **Run workflow**
en la web aparece cuando el archivo está en la rama predeterminada. El workflow
de Surco ya está integrado en `main`.
([Eventos GitHub](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_dispatch),
[CLI oficial](https://cli.github.com/manual/gh_workflow_run))

Al ejecutar el workflow, elegí la rama revisada:

1. Con `upload` desactivado, se consulta el registro de la app en Apple y se genera
   un IPA firmado revisable en el artefacto `surco-testflight`.
2. Con `upload` activado, también se ejecutan la validación Apple y la subida.

El script reutiliza el motor Rust y su empaquetado iOS, archiva con firma manual
y exporta con el método `app-store-connect`. Cada ejecución usa un build
`GITHUB_RUN_NUMBER.GITHUB_RUN_ATTEMPT`. Para subir un build posterior, iniciá una
ejecución nueva; repetir una ejecución vieja puede tener un número inferior al
último que Apple recibió.

El certificado, el perfil y la clave API se materializan en un directorio
temporal privado fuera de los artefactos. El keychain temporal y el perfil que
instala esa ejecución se eliminan en `finally`, y se restaura la lista previa
de keychains. Los logs guardados redactan las credenciales. El IPA contiene los
metadatos públicos de firma necesarios para distribuirlo. Si un runner alojado
es cancelado a la fuerza, GitHub descarta la máquina al finalizar el job.

La subida usa `altool` con la clave `.p8` bajo `API_PRIVATE_KEYS_DIR`, mecanismo
documentado por Apple. Una subida exitosa inicia el procesamiento; el resultado
local no confirma que Apple ya haya habilitado el build.
([Upload oficial](https://developer.apple.com/help/app-store-connect/manage-builds/upload-builds/),
[manual altool](https://help.apple.com/asc/appsaltool/en.lproj/apdATD1E53-D1E1A1303-D1E53A1126.html))

## 5. Instalar en el iPhone

La distribución actual autorizada es **fuera de Francia**. La app incorpora TLS
mediante rustls (`ring` y `aws-lc-rs`), además de TOTP/HMAC-SHA1 y hashes SHA256:
usa algoritmos estándar incluidos en la app, no solo el cifrado del SO Apple.
Apple distingue ese caso del cifrado limitado al sistema operativo y señala la
declaración francesa para distribución por App Store en Francia. Antes de ampliar
la distribución a ese país, revisá la clasificación y documentación aplicables.
([Documentación de cifrado de Apple](https://developer.apple.com/help/app-store-connect/reference/app-information/export-compliance-documentation-for-encryption/))

1. En **App Store Connect → Surco Music → TestFlight**, esperá que termine el procesamiento.
2. Completá export compliance y la información de pruebas que Apple solicite.
   Respondé según las integraciones reales de la app; el workflow no declara
   automáticamente exenciones ni las respuestas de App Privacy.
3. Asigná el build manualmente al grupo **Surco interno** para que lo reciban
   sus usuarios. La distribución automática de ese grupo está desactivada.
4. Instalá la app TestFlight de Apple en tu iPhone, aceptá la invitación y tocá
   **Instalar** en Surco; para la instalación existente, tocá **Actualizar**.

Los builds duran 90 días. La prueba interna admite hasta 100 usuarios de App
Store Connect; invitar testers externos añade Beta App Review y sus requisitos.
([TestFlight oficial](https://developer.apple.com/help/app-store-connect/test-a-beta-version/testflight-overview/))

El proyecto incluye el AppIcon de Surco y el manifest de razones de API para
metadatos de archivos dentro del contenedor y temporizadores/intervalos propios.
El manifest no reemplaza la declaración de datos de App Privacy.
([Razones Apple](https://developer.apple.com/documentation/bundleresources/app-privacy-configuration/nsprivacyaccessedapitypes/nsprivacyaccessedapitype))

En cada beta nueva, revalidá audio, Bluetooth/AirPlay, llamadas/interrupciones,
bloqueo de pantalla y reproducción en segundo plano, además de los gestos y
accesibilidad modificados. El usuario ya confirmó segundo plano/bloqueo en
**0.2.0 (3.1)**; la comprobación física de **0.3.0** sigue pendiente. El CI de
simulador aporta evidencia de arranque e interfaz; las funciones de hardware
necesitan el iPhone real.
