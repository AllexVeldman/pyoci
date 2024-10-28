# Changelog

## [0.1.12](https://github.com/AllexVeldman/pyoci/compare/v0.1.11...v0.1.12) (2024-10-28)


### Miscellaneous Chores

* **deps:** bump anyhow from 1.0.90 to 1.0.91 ([#103](https://github.com/AllexVeldman/pyoci/issues/103)) ([d9e06d8](https://github.com/AllexVeldman/pyoci/commit/d9e06d86d401485066f94a1d18b652fe77175f32))
* **deps:** bump bytes from 1.7.2 to 1.8.0 ([#100](https://github.com/AllexVeldman/pyoci/issues/100)) ([a1a2952](https://github.com/AllexVeldman/pyoci/commit/a1a2952986383746fb893ff1e4f90ce97705eae1))
* **deps:** bump pin-project from 1.1.6 to 1.1.7 ([#104](https://github.com/AllexVeldman/pyoci/issues/104)) ([452cbc7](https://github.com/AllexVeldman/pyoci/commit/452cbc79b4bf90d825184385e9bc8055ccc7bb4c))
* **deps:** bump serde from 1.0.210 to 1.0.213 ([#101](https://github.com/AllexVeldman/pyoci/issues/101)) ([f4d918a](https://github.com/AllexVeldman/pyoci/commit/f4d918acc8a892494b9b84a1d9f8515e5444410c))
* **deps:** bump tokio from 1.40.0 to 1.41.0 ([#102](https://github.com/AllexVeldman/pyoci/issues/102)) ([cb607da](https://github.com/AllexVeldman/pyoci/commit/cb607da78b3331e6227be82ee829eaeb986690ca))

## [0.1.11](https://github.com/AllexVeldman/pyoci/compare/v0.1.10...v0.1.11) (2024-10-21)


### Features

* Instruct caches for root and unmatched routes ([#98](https://github.com/AllexVeldman/pyoci/issues/98)) ([f27c3c1](https://github.com/AllexVeldman/pyoci/commit/f27c3c102e660f6546af07fd78acbfa612d743c4))


### Miscellaneous Chores

* **deps:** bump anyhow from 1.0.89 to 1.0.90 ([#96](https://github.com/AllexVeldman/pyoci/issues/96)) ([5b8bcdb](https://github.com/AllexVeldman/pyoci/commit/5b8bcdb6137a172d39407ac51ca013bb3f24a7c4))
* **deps:** bump futures from 0.3.30 to 0.3.31 ([#94](https://github.com/AllexVeldman/pyoci/issues/94)) ([ac5a7d3](https://github.com/AllexVeldman/pyoci/commit/ac5a7d38ac3d42e9da35da47fc101d49eac73aae))
* **deps:** bump pin-project from 1.1.5 to 1.1.6 ([#97](https://github.com/AllexVeldman/pyoci/issues/97)) ([4861163](https://github.com/AllexVeldman/pyoci/commit/486116334812212d7b540f96fdcb2975139707dc))
* **deps:** bump serde_json from 1.0.128 to 1.0.132 ([#95](https://github.com/AllexVeldman/pyoci/issues/95)) ([69deb3c](https://github.com/AllexVeldman/pyoci/commit/69deb3cba16c1a3714a351eaa3138d310b3e7fbf))

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
