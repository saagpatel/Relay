# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Security

- Protocol 3.0.0: every peer message is sealed under per-direction HKDF keys with a counter nonce, and key confirmation is bound to role and direction. An on-path attacker without the code could previously forge a "verified" zero-length transfer, fake delivery confirmations, and pass key confirmation by reflection. 2.x clients and servers are refused. Formal model in `formal/`.

- Initial release
