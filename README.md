# RLauncher

Лаунчер для Minecraft: Java Edition на Rust (egui).

## Сборка на Windows
1. Установите Rust: https://rustup.rs (нужен MSVC build tools).
2. В папке проекта: `cargo run --release`
   Готовый exe: `target\release\rlauncher.exe`

## Что есть
- Заставка загрузки, главное меню, каталог RL Mods (Modrinth), настройки.
- «Версии»: создание версии (название, Vanilla/Forge/Fabric/Quilt/NeoForge, версия игры от старых к новым).
- «Аккаунты»: офлайн-ник (можно на серверы с online-mode=false) и вход через Microsoft.
- Каталог: моды, сборки (.mrpack), ресурс-паки, шейдеры, датапаки; файл подбирается под версию игры и загрузчик, обязательные зависимости ставятся сами.
- Java скачивается автоматически (официальная среда Mojang).

## Вход через Microsoft
Нужен ваш Azure Client ID (Настройки -> Настройки RLauncher). Приложение должно быть одобрено Mojang (aka.ms/AppRegInfo).
Без него работает офлайн-режим.
