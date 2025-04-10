# Changelog

## [0.1.26](https://github.com/AllexVeldman/pyoci/compare/v0.1.25...v0.1.26) (2025-04-10)


### Code Refactoring

* Consolidate unix timestamp nano into the time module ([#200](https://github.com/AllexVeldman/pyoci/issues/200)) ([21c768e](https://github.com/AllexVeldman/pyoci/commit/21c768e7a639fe6d170cfc27c4d62ad65fad8128))
* Move getting UTC time to a separate module ([#199](https://github.com/AllexVeldman/pyoci/issues/199)) ([7c1f9d7](https://github.com/AllexVeldman/pyoci/commit/7c1f9d7b2162483dd8cb47c6a205e96b088c2cab))


### Dependency Updates

* Bump bytes from 1.10.0 to 1.10.1 ([#191](https://github.com/AllexVeldman/pyoci/issues/191)) ([91bdcf9](https://github.com/AllexVeldman/pyoci/commit/91bdcf9596aac8c94c2410c6eef464729da03c6f))
* Bump serde from 1.0.217 to 1.0.219 ([#190](https://github.com/AllexVeldman/pyoci/issues/190)) ([a532878](https://github.com/AllexVeldman/pyoci/commit/a5328789cff840b8fe5ac7bc0c2a12a93c5e0c64))
* bump time from 0.3.37 to 0.3.41 ([#196](https://github.com/AllexVeldman/pyoci/issues/196)) ([a62c0db](https://github.com/AllexVeldman/pyoci/commit/a62c0db6bfa914814c378cd2a226403347517868))
* bump tokio from 1.44.0 to 1.44.2 ([#197](https://github.com/AllexVeldman/pyoci/issues/197)) ([640b65b](https://github.com/AllexVeldman/pyoci/commit/640b65b2cb5a4e44aad5f6aeaafb1c4bcfc280d2))
* Bump tokio-util from 0.7.13 to 0.7.14 ([#188](https://github.com/AllexVeldman/pyoci/issues/188)) ([16b6fac](https://github.com/AllexVeldman/pyoci/commit/16b6fac95be43677ee2891f47c8fafc14805c555))

## [0.1.25](https://github.com/AllexVeldman/pyoci/compare/v0.1.24...v0.1.25) (2025-03-18)


### Bug Fixes

* `project_urls` not in correct location ([#194](https://github.com/AllexVeldman/pyoci/issues/194)) ([40455a0](https://github.com/AllexVeldman/pyoci/commit/40455a0c8f458fcb379573bb96c04acb7336c2fd))

## [0.1.24](https://github.com/AllexVeldman/pyoci/compare/v0.1.23...v0.1.24) (2025-03-17)


### Features

* Include `project_urls` in `<package>/json` ([#192](https://github.com/AllexVeldman/pyoci/issues/192)) ([68e1fed](https://github.com/AllexVeldman/pyoci/commit/68e1fedf50f91ef8db7e5d3150262cc77422d823))

## [0.1.23](https://github.com/AllexVeldman/pyoci/compare/v0.1.22...v0.1.23) (2025-03-11)


### Dependency Updates

* Bump mockito from 1.6.1 to 1.7.0 ([#184](https://github.com/AllexVeldman/pyoci/issues/184)) ([b559ebd](https://github.com/AllexVeldman/pyoci/commit/b559ebd85b4782f03a3094359aa0a49eafa18cb5))
* Bump pin-project from 1.1.9 to 1.1.10 ([#182](https://github.com/AllexVeldman/pyoci/issues/182)) ([4c46766](https://github.com/AllexVeldman/pyoci/commit/4c4676669fda8fcdb53868e2fd033275b557db54))
* Bump ring from 0.17.8 to 0.17.13 in the cargo group ([#180](https://github.com/AllexVeldman/pyoci/issues/180)) ([360a256](https://github.com/AllexVeldman/pyoci/commit/360a256503e04cd9113f47d9c316435c4a2c564c))
* Bump serde_json from 1.0.139 to 1.0.140 ([#183](https://github.com/AllexVeldman/pyoci/issues/183)) ([9d975d3](https://github.com/AllexVeldman/pyoci/commit/9d975d3c9b70643849b45c96b7575748dab25471))
* Bump tokio from 1.43.0 to 1.44.0 ([#181](https://github.com/AllexVeldman/pyoci/issues/181)) ([381555c](https://github.com/AllexVeldman/pyoci/commit/381555c3a95e99ef8a3a4c5edb35afc9ec2809b2))

## [0.1.22](https://github.com/AllexVeldman/pyoci/compare/v0.1.21...v0.1.22) (2025-03-03)


### Bug Fixes

* Breaking changes in Axum ([#154](https://github.com/AllexVeldman/pyoci/issues/154)) ([9372d1b](https://github.com/AllexVeldman/pyoci/commit/9372d1b2eca224249cab8030434bd66b41b94ee7))


### Miscellaneous Chores

* **release:** Give dependency updates their own section in changelog ([#177](https://github.com/AllexVeldman/pyoci/issues/177)) ([e606350](https://github.com/AllexVeldman/pyoci/commit/e606350fe104ddeed13f4fa6826c135baae992fd))
* Switch console log to a more compact format ([#174](https://github.com/AllexVeldman/pyoci/issues/174)) ([7b90e86](https://github.com/AllexVeldman/pyoci/commit/7b90e86cf6756ffa8286c893b1248f041a962f31))


### Dependency Updates

* bump async-trait from 0.1.85 to 0.1.87 ([#176](https://github.com/AllexVeldman/pyoci/issues/176)) ([f1d7eb7](https://github.com/AllexVeldman/pyoci/commit/f1d7eb777acd09164857ae61462ef329db4097fd))
* bump axum from 0.7.9 to 0.8.1 ([#154](https://github.com/AllexVeldman/pyoci/issues/154)) ([9372d1b](https://github.com/AllexVeldman/pyoci/commit/9372d1b2eca224249cab8030434bd66b41b94ee7))
* bump bytes from 1.9.0 to 1.10.0 ([#171](https://github.com/AllexVeldman/pyoci/issues/171)) ([45eb946](https://github.com/AllexVeldman/pyoci/commit/45eb946e1e326cc0e295ffdff3feb69c5eac42dd))
* bump pin-project from 1.1.8 to 1.1.9 ([#172](https://github.com/AllexVeldman/pyoci/issues/172)) ([4921104](https://github.com/AllexVeldman/pyoci/commit/49211044307bd7adc569232c323650cc06610c18))
* bump prost from 0.13.4 to 0.13.5 ([#179](https://github.com/AllexVeldman/pyoci/issues/179)) ([a124bdb](https://github.com/AllexVeldman/pyoci/commit/a124bdb3fea0469418c082b4a927916efac91c73))
* bump serde_json from 1.0.137 to 1.0.139 ([#173](https://github.com/AllexVeldman/pyoci/issues/173)) ([45edf47](https://github.com/AllexVeldman/pyoci/commit/45edf47dc077794e09eb8319c9fcbc84c172390a))
* bump the opentelemetry group with 3 updates ([#178](https://github.com/AllexVeldman/pyoci/issues/178)) ([2913720](https://github.com/AllexVeldman/pyoci/commit/2913720bdc26b0979a0bc961cf5dd3e851db2dde))

## [0.1.21](https://github.com/AllexVeldman/pyoci/compare/v0.1.20...v0.1.21) (2025-02-06)


### Bug Fixes

* Package version list incomplete ([#167](https://github.com/AllexVeldman/pyoci/issues/167)) ([1660245](https://github.com/AllexVeldman/pyoci/commit/1660245b1a0c8c5fc918b8040b40b5241749d56e)), closes [#166](https://github.com/AllexVeldman/pyoci/issues/166)

## [0.1.20](https://github.com/AllexVeldman/pyoci/compare/v0.1.19...v0.1.20) (2025-01-31)


### Features

* Add the package digest to `list_package` ([#163](https://github.com/AllexVeldman/pyoci/issues/163)) ([7fd3ccd](https://github.com/AllexVeldman/pyoci/commit/7fd3ccd4157225611934369e2260399da09bd9ab)), closes [#160](https://github.com/AllexVeldman/pyoci/issues/160)
* Allow setting labels on the OCI image ([#159](https://github.com/AllexVeldman/pyoci/issues/159)) ([6140d20](https://github.com/AllexVeldman/pyoci/commit/6140d200529d80286fb6f8ac347fd75e888cd8ab))
* SHA256 digest of the provided content is checked against the provided `sha256_digest` in the request. [#162](https://github.com/AllexVeldman/pyoci/issues/162) ([968afe8](https://github.com/AllexVeldman/pyoci/commit/968afe89ed540f81fcb8bd0994d2e7938741ab7f))


### Miscellaneous Chores

* **deps:** bump pin-project from 1.1.7 to 1.1.8 ([#157](https://github.com/AllexVeldman/pyoci/issues/157)) ([8d9d0d2](https://github.com/AllexVeldman/pyoci/commit/8d9d0d27e7974e91efce5cc8a3429f891c2b987e))
* **deps:** bump reqwest from 0.12.9 to 0.12.12 ([#156](https://github.com/AllexVeldman/pyoci/issues/156)) ([b0160cb](https://github.com/AllexVeldman/pyoci/commit/b0160cb897145ee2ccee08ee6511f11d49d50361))
* **deps:** bump serde_json from 1.0.135 to 1.0.137 ([#155](https://github.com/AllexVeldman/pyoci/issues/155)) ([ed7f932](https://github.com/AllexVeldman/pyoci/commit/ed7f932484de0cdc2005f9af8d65284ed006b510))


### Code Refactoring

* Refactor of the publish path and HttpTransport [#162](https://github.com/AllexVeldman/pyoci/issues/162) ([968afe8](https://github.com/AllexVeldman/pyoci/commit/968afe89ed540f81fcb8bd0994d2e7938741ab7f))

## [0.1.19](https://github.com/AllexVeldman/pyoci/compare/v0.1.18...v0.1.19) (2025-01-13)


### Features

* Allow configuring the max body size ([#153](https://github.com/AllexVeldman/pyoci/issues/153)) ([852818f](https://github.com/AllexVeldman/pyoci/commit/852818fe8ef4001e37a6e008b21fa06dab378246))


### Bug Fixes

* Return HTTP 413 Payload Too Large instead of 500 ([852818f](https://github.com/AllexVeldman/pyoci/commit/852818fe8ef4001e37a6e008b21fa06dab378246))


### Documentation

* Add CONTRIBUTING and examples README ([#146](https://github.com/AllexVeldman/pyoci/issues/146)) ([7a0d8ab](https://github.com/AllexVeldman/pyoci/commit/7a0d8abef2ee480c3fa140f45830292e58dad48e))


### Miscellaneous Chores

* **deps:** bump async-trait from 0.1.84 to 0.1.85 ([#148](https://github.com/AllexVeldman/pyoci/issues/148)) ([bd0000d](https://github.com/AllexVeldman/pyoci/commit/bd0000d4e674502f183726ab20c7d347db6d8006))
* **deps:** bump bytes from 1.8.0 to 1.9.0 ([#150](https://github.com/AllexVeldman/pyoci/issues/150)) ([5d0b3f7](https://github.com/AllexVeldman/pyoci/commit/5d0b3f755724996af2c25c0724fbb60be9e67a37))
* **deps:** bump serde from 1.0.215 to 1.0.217 ([#152](https://github.com/AllexVeldman/pyoci/issues/152)) ([56fa102](https://github.com/AllexVeldman/pyoci/commit/56fa102d2a2432d12e66d6ad236db78932589ba8))
* **deps:** bump serde_json from 1.0.133 to 1.0.135 ([#149](https://github.com/AllexVeldman/pyoci/issues/149)) ([4022267](https://github.com/AllexVeldman/pyoci/commit/4022267194fbb2cafa30557f156efd9dfde46da2))
* **deps:** bump tokio from 1.41.1 to 1.43.0 ([#145](https://github.com/AllexVeldman/pyoci/issues/145)) ([2984aa8](https://github.com/AllexVeldman/pyoci/commit/2984aa8ee31b3f4cd9f4ab6b9d93e1edb89e56f0))
* **deps:** bump tower from 0.5.1 to 0.5.2 ([#151](https://github.com/AllexVeldman/pyoci/issues/151)) ([beddeb2](https://github.com/AllexVeldman/pyoci/commit/beddeb2f0ba5ed10b23f798b1883850df130a4e8))

## [0.1.18](https://github.com/AllexVeldman/pyoci/compare/v0.1.17...v0.1.18) (2025-01-09)


### Features

* Allow hosting PyOCI on a subpath ([#142](https://github.com/AllexVeldman/pyoci/issues/142)) ([328044e](https://github.com/AllexVeldman/pyoci/commit/328044e9b4bb09af67f2237c1fb8c29dfde172e8)), closes [#141](https://github.com/AllexVeldman/pyoci/issues/141)
* Health endpoint ([#143](https://github.com/AllexVeldman/pyoci/issues/143)) ([090f520](https://github.com/AllexVeldman/pyoci/commit/090f5202886ca4110db7016e531d0f96c71f6452))


### Miscellaneous Chores

* **deps:** bump async-trait from 0.1.83 to 0.1.84 ([#136](https://github.com/AllexVeldman/pyoci/issues/136)) ([e00e5c0](https://github.com/AllexVeldman/pyoci/commit/e00e5c0d2353e522514705f2b6c9e6a5d1b94003))
* **deps:** bump prost from 0.13.3 to 0.13.4 ([#139](https://github.com/AllexVeldman/pyoci/issues/139)) ([75aa552](https://github.com/AllexVeldman/pyoci/commit/75aa552e26b8f0e0abd178f133ed3a2ed0e1a8e9))
* **deps:** bump tokio-util from 0.7.12 to 0.7.13 ([#138](https://github.com/AllexVeldman/pyoci/issues/138)) ([821c3c4](https://github.com/AllexVeldman/pyoci/commit/821c3c427efd29b86b574f28147b8a1929cd4d89))
* **deps:** bump tracing-subscriber from 0.3.18 to 0.3.19 ([#140](https://github.com/AllexVeldman/pyoci/issues/140)) ([5f886e0](https://github.com/AllexVeldman/pyoci/commit/5f886e07a582c1665c94baab6ecaada90dc7a3e8))

## [0.1.17](https://github.com/AllexVeldman/pyoci/compare/v0.1.16...v0.1.17) (2024-12-30)


### Miscellaneous Chores

* **deps:** bump anyhow from 1.0.93 to 1.0.95 ([#134](https://github.com/AllexVeldman/pyoci/issues/134)) ([8ade5e2](https://github.com/AllexVeldman/pyoci/commit/8ade5e2a90688ca97560d132295e8c45988decf2))
* **deps:** bump http from 1.1.0 to 1.2.0 ([#131](https://github.com/AllexVeldman/pyoci/issues/131)) ([e1fdcd9](https://github.com/AllexVeldman/pyoci/commit/e1fdcd98ee5d445bb18f8872dd4c8c8fef3f11a2))
* **deps:** bump the opentelemetry group with 2 updates ([#129](https://github.com/AllexVeldman/pyoci/issues/129)) ([4205cc8](https://github.com/AllexVeldman/pyoci/commit/4205cc8be04f8760a12dd053fe6076c8786b4b72))
* **deps:** bump time from 0.3.36 to 0.3.37 ([#132](https://github.com/AllexVeldman/pyoci/issues/132)) ([eaa5ca3](https://github.com/AllexVeldman/pyoci/commit/eaa5ca3f82fb36ff66b6252825a39c6d66b847bc))
* **deps:** bump tracing from 0.1.40 to 0.1.41 ([#130](https://github.com/AllexVeldman/pyoci/issues/130)) ([de3309b](https://github.com/AllexVeldman/pyoci/commit/de3309b0dff96f4c577893a83e28d19afe7fa9d0))

## [0.1.16](https://github.com/AllexVeldman/pyoci/compare/v0.1.15...v0.1.16) (2024-12-07)


### Miscellaneous Chores

* **trace:** Add trace attributes ([#127](https://github.com/AllexVeldman/pyoci/issues/127)) ([7ead818](https://github.com/AllexVeldman/pyoci/commit/7ead818f09f5990969da8b8bfb18834510dc2dc3))

## [0.1.15](https://github.com/AllexVeldman/pyoci/compare/v0.1.14...v0.1.15) (2024-11-25)


### Miscellaneous Chores

* Add created timestamp to manifest ([#123](https://github.com/AllexVeldman/pyoci/issues/123)) ([fec6287](https://github.com/AllexVeldman/pyoci/commit/fec6287df33768263eae13bde6f8c83bc6401048))
* **deps:** bump url from 2.5.3 to 2.5.4 ([#126](https://github.com/AllexVeldman/pyoci/issues/126)) ([3319f9c](https://github.com/AllexVeldman/pyoci/commit/3319f9c61d65a07af794bd9390caef53681d3dcd))
* **tests:** Add test for adding a file to existing index ([#125](https://github.com/AllexVeldman/pyoci/issues/125)) ([40e3c11](https://github.com/AllexVeldman/pyoci/commit/40e3c115aa3747400dece4db0f53eeeae6450be6))

## [0.1.14](https://github.com/AllexVeldman/pyoci/compare/v0.1.13...v0.1.14) (2024-11-20)


### Bug Fixes

* Return 400 BAD_REQUEST for invalid filenames ([#122](https://github.com/AllexVeldman/pyoci/issues/122)) ([48d15c3](https://github.com/AllexVeldman/pyoci/commit/48d15c339227f01796fc6ea8d679088ceffb83ca))


### Miscellaneous Chores

* Include creation date in image index ([#120](https://github.com/AllexVeldman/pyoci/issues/120)) ([37b0b3b](https://github.com/AllexVeldman/pyoci/commit/37b0b3b1cbe1c91782158b076961079172645164))

## [0.1.13](https://github.com/AllexVeldman/pyoci/compare/v0.1.12...v0.1.13) (2024-11-18)


### Documentation

* Add Azure Container Registry to tested registries ([c349a21](https://github.com/AllexVeldman/pyoci/commit/c349a21fe989a535304d91f5407b4790c398c980))
* Update delete section ([#119](https://github.com/AllexVeldman/pyoci/issues/119)) ([a881a11](https://github.com/AllexVeldman/pyoci/commit/a881a11f2cb983238f4a0c427cb943054a5376c2))


### Miscellaneous Chores

* **deps:** bump anyhow from 1.0.91 to 1.0.93 ([#112](https://github.com/AllexVeldman/pyoci/issues/112)) ([dce3d0c](https://github.com/AllexVeldman/pyoci/commit/dce3d0cba57dbe41e91c6df04c86ed20a011b8bd))
* **deps:** bump axum from 0.7.7 to 0.7.9 ([#116](https://github.com/AllexVeldman/pyoci/issues/116)) ([3292be3](https://github.com/AllexVeldman/pyoci/commit/3292be3a9aab955957c3d8527e4e50e44c87c1aa))
* **deps:** bump mockito from 1.5.0 to 1.6.1 ([#117](https://github.com/AllexVeldman/pyoci/issues/117)) ([46b3556](https://github.com/AllexVeldman/pyoci/commit/46b35569c97fcf4494bb5aeb3667c04ffdbfa4e6))
* **deps:** bump oci-spec from 0.7.0 to 0.7.1 ([#118](https://github.com/AllexVeldman/pyoci/issues/118)) ([2776be5](https://github.com/AllexVeldman/pyoci/commit/2776be5e1c08f7a948729c05284a9be7e42e289f))
* **deps:** bump reqwest from 0.12.8 to 0.12.9 ([#107](https://github.com/AllexVeldman/pyoci/issues/107)) ([4e063c1](https://github.com/AllexVeldman/pyoci/commit/4e063c18cf64feb7c83cc41c211a741346b49f0b))
* **deps:** bump serde from 1.0.213 to 1.0.215 ([#113](https://github.com/AllexVeldman/pyoci/issues/113)) ([3915668](https://github.com/AllexVeldman/pyoci/commit/39156681378cf33fbf5c437f79f692ffff783ad6))
* **deps:** bump serde_json from 1.0.132 to 1.0.133 ([#115](https://github.com/AllexVeldman/pyoci/issues/115)) ([9092c3b](https://github.com/AllexVeldman/pyoci/commit/9092c3bbe36ed64577769109aa1116f739d7e26d))
* **deps:** bump the opentelemetry group with 3 updates ([#114](https://github.com/AllexVeldman/pyoci/issues/114)) ([e13b424](https://github.com/AllexVeldman/pyoci/commit/e13b42475930e13ceda0ea1c293c4c865f44bd3e))
* **deps:** bump tokio from 1.41.0 to 1.41.1 ([#110](https://github.com/AllexVeldman/pyoci/issues/110)) ([7463838](https://github.com/AllexVeldman/pyoci/commit/74638385da5bb34cb5b547d7c25281c9ca4de876))
* **deps:** bump url from 2.5.2 to 2.5.3 ([#111](https://github.com/AllexVeldman/pyoci/issues/111)) ([f7df494](https://github.com/AllexVeldman/pyoci/commit/f7df4949f2b13538abcdbdd9746c2eaa606108c5))

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
