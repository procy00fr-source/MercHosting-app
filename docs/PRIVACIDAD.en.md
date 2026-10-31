# MercHosting privacy policy

Last updated: October 31, 2026.

MercHosting is a Windows desktop app that creates and manages Minecraft servers **on your own PC**. It has no accounts, no central MercHosting server and **sends no usage statistics or telemetry**.

## What is stored and where

Everything stays on your PC:

- **App data** in `%LOCALAPPDATA%\MercHosting`: your server list, settings, Activity history, console logs, backups, the MercHosting trash, the modpack record and, if you're a Supporter, your license key.
- **New servers**, by default in `%USERPROFILE%\MercHosting\Servidores` (or the folder you choose).
- Interface preferences (language, theme, console favorites) in the window's local storage.

You can delete all of it by uninstalling the app and deleting those folders. MercHosting never deletes a server unless you ask: when you remove one, its folder goes to the Recycle Bin.

## What it connects to and why

MercHosting only connects to the Internet to do what you ask. In each case the service receives your IP address and the request, like any download:

| Service | When | What is sent |
|---|---|---|
| Mojang, PaperMC, Purpur, Fabric, Quilt, Forge, NeoForge, SpigotMC (BuildTools), GitHub (Arclight) | When creating or updating a server | The version you ask for |
| Eclipse Adoptium (Temurin) | If you install Java from the app | The Java version |
| Modrinth, CurseForge, Hangar | When searching, installing or updating mods, plugins, datapacks, modpacks or resource packs, and when checking installed content for updates | What you search for and the hashes (SHA-1) of installed files to recognize them |
| Playit.gg | Only if you turn on Internet access | What's needed to create your tunnel; the account and agent belong to Playit and have their own policy |
| GitHub (MercHosting releases repository) | When checking for app updates | Nothing beyond the request |
| Gumroad | Only if you activate Supporter Edition | Your license key, to verify it |
| iroh relay (n0) | Only if you use Companions and there's no direct connection | End-to-end encrypted traffic between your PCs; the relay can't read it |
| Google Forms | Only if you send feedback | The form opens in your browser with what you wrote and the app version; you decide whether to send it |
| The address you set as a resource pack | When checking it | One download of the file to compute its hash |

**Companions:** the connection between PCs is direct and encrypted (iroh). A companion can only do what you allowed on each server; they never run programs on your PC.

**Your players:** they connect to your Minecraft server, not to MercHosting. Player history (name, first and last connection, play time and your private notes) is stored only on your PC, from your own server's logs.

## What we don't do

- We don't sell or share data.
- There are no ads or trackers.
- Your worlds, files and logs aren't uploaded anywhere unless you export or share them.

## Contact

Use the app's **Feedback** section or the author's channels (YouTube @Amercson, TikTok @amercfr).

If this policy changes, the date above will change and we'll mention it in the release notes.
