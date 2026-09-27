# Full-flight stutter comparison

Full-flight comparison complete; inspect paired rates and retained per-flight variation.

Primary window starts five seconds after the first unpaused callback. Activation and full-flight results are retained separately. These are native callback intervals, not display-present or VR-compositor timings.

| Run / flight | Flag | Valid | Seconds | >25 ms | >50 ms | >100 ms | p99.9 ms | Max ms |
|---|---|---|---:|---:|---:|---:|---:|---:|
| XPT_FULL_FLIGHT_AB02_20260914_1_0 / calm-01 | False | True | 155.6339245 | 26 | 3 | 2 | 29.141865300000983 | 583.4809 |
| XPT_FULL_FLIGHT_AB02_20260914_1_0 / calm-02 | False | True | 155.0632797 | 39 | 5 | 1 | 40.289100000000005 | 101.3585 |
| XPT_FULL_FLIGHT_AB02_20260914_1_1 / calm-01 | True | True | 155.268937 | 21 | 3 | 0 | 30.230267599999735 | 59.8928 |
| XPT_FULL_FLIGHT_AB02_20260914_1_1 / calm-02 | True | True | 154.9814259 | 41 | 1 | 0 | 37.54775749999991 | 54.0359 |
| XPT_FULL_FLIGHT_AB02_20260914_2_1 / calm-01 | True | True | 155.2219668 | 17 | 0 | 0 | 26.19795280000001 | 34.048 |
| XPT_FULL_FLIGHT_AB02_20260914_2_1 / calm-02 | True | True | 154.69267739999998 | 58 | 16 | 8 | 61.67692960001122 | 195.4837 |
| XPT_FULL_FLIGHT_AB02_20260914_2_0 / calm-01 | False | True | 156.2644396 | 17 | 2 | 2 | 25.55286799999992 | 605.4587 |
| XPT_FULL_FLIGHT_AB02_20260914_2_0 / calm-02 | False | True | 154.7479174 | 52 | 15 | 2 | 58.72615380000003 | 349.47889999999995 |
| XPT_FULL_FLIGHT_AB02_20260914_3_0 / calm-01 | False | True | 158.96456030000002 | 38 | 7 | 7 | 28.58804520000122 | 809.8973000000001 |
| XPT_FULL_FLIGHT_AB02_20260914_3_0 / calm-02 | False | True | 155.0278414 | 43 | 4 | 0 | 40.71937120000018 | 95.1297 |
| XPT_FULL_FLIGHT_AB02_20260914_3_1 / calm-01 | True | True | 155.1004381 | 22 | 5 | 3 | 41.659880399999686 | 161.26940000000002 |
| XPT_FULL_FLIGHT_AB02_20260914_3_1 / calm-02 | True | True | 155.02129190000002 | 41 | 1 | 0 | 38.68129740000011 | 52.4572 |
