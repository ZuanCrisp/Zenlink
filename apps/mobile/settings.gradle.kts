// ZenLink Mobile + TV: one Android app with an adaptive UI (phone, tablet, TV).
// Design notes: ZenshiroDev/docs/MOBILE.md.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}
rootProject.name = "ZenLinkMobile"
include(":app")
