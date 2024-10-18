# Changelog

## [0.1.10](https://github.com/AllexVeldman/pyoci/compare/v0.1.9...v0.1.10) (2024-10-18)


### Features

* **OTLP:** Add requests count metric ([#92](https://github.com/AllexVeldman/pyoci/issues/92)) ([289d316](https://github.com/AllexVeldman/pyoci/commit/289d316e25bea262db3571dcc1ac74e2560e318c))

## [0.1.9](https://github.com/AllexVeldman/pyoci/compare/v0.1.8...v0.1.9) (2024-10-18)


### Features

* **OTLP:** Collect uptime metrics ([#90](https://github.com/AllexVeldman/pyoci/issues/90)) ([de8a06c](https://github.com/AllexVeldman/pyoci/commit/de8a06c53f3e1a201fdec3b914e77186f9aa4fdb))

## [0.1.8](https://github.com/AllexVeldman/pyoci/compare/v0.1.7...v0.1.8) (2024-10-17)


### Features

* **log:** Include host is logs ([#88](https://github.com/AllexVeldman/pyoci/issues/88)) ([c65c0b6](https://github.com/AllexVeldman/pyoci/commit/c65c0b62dafbddf8e3a862db36a06d8cfbfe8e32))
* **OTLP:** Include version in OTLP logs/traces ([#84](https://github.com/AllexVeldman/pyoci/issues/84)) ([d9d782f](https://github.com/AllexVeldman/pyoci/commit/d9d782fc88410d4bab4136b70bcb006f2475e202))


### Bug Fixes

* **auth:** Log a warning when no auth is provided ([#86](https://github.com/AllexVeldman/pyoci/issues/86)) ([3362f4d](https://github.com/AllexVeldman/pyoci/commit/3362f4d1d3068e8827d8b6920687df109c14a8b5))


### Code Refactoring

* Remove use of Regex ([#87](https://github.com/AllexVeldman/pyoci/issues/87)) ([98708e1](https://github.com/AllexVeldman/pyoci/commit/98708e1357a065d618242530d6a6cd8805fdeaab))


### Build System

* Use Github cache for docker build ([#89](https://github.com/AllexVeldman/pyoci/issues/89)) ([375b810](https://github.com/AllexVeldman/pyoci/commit/375b81059d96afb36914ba4b8628d0eb25803bb2))

## [0.1.7](https://github.com/AllexVeldman/pyoci/compare/v0.1.6...v0.1.7) (2024-10-17)


### Continuous Integration

* **release:** Can't rely on workflow booleans ([ffef246](https://github.com/AllexVeldman/pyoci/commit/ffef2465f4f98c32a237047fe64157e68da8bf24))
* **release:** Fix publish output not being a boolean ([#82](https://github.com/AllexVeldman/pyoci/issues/82)) ([956924d](https://github.com/AllexVeldman/pyoci/commit/956924d9acc69858b6762354ca9c49491c9d3805))

## [0.1.6](https://github.com/AllexVeldman/pyoci/compare/0.1.5...v0.1.6) (2024-10-17)


### Features

* **auth:** Include `scope` in the token exchange ([#75](https://github.com/AllexVeldman/pyoci/issues/75)) ([1a17f48](https://github.com/AllexVeldman/pyoci/commit/1a17f4803eafb78ba1a393864ef5be070b3c872d))


### Documentation

* Add links to specs ([b4a4802](https://github.com/AllexVeldman/pyoci/commit/b4a480274df9e0079e1e69e57efd8ca34e9404fc))


### Build System

* Include license in docker image ([#81](https://github.com/AllexVeldman/pyoci/issues/81)) ([6fb4c1c](https://github.com/AllexVeldman/pyoci/commit/6fb4c1c099eba1797df548337910cd4a97bc4017))


### Continuous Integration

* **release:** Configure release-please ([#74](https://github.com/AllexVeldman/pyoci/issues/74)) ([1c30d98](https://github.com/AllexVeldman/pyoci/commit/1c30d98521c698455a98b6cc0f18cd74287bac80))
