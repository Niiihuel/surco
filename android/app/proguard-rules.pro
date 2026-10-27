# The verifier's Kotlin half is called over JNI, which R8 cannot see.
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }

# Rust finds the bridge by its name and calls into it over JNI.
-keep class io.github.niiihuel.surco.SurcoBridge { *; }
