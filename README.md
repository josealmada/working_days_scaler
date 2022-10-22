## working_days_scaler

External scaler for KEDA that scales based on the nth working day of the month

## Dependencies

* protobuf-compiler
* libprotobuf-dev
* cmake

## Compile

Clone
```shell
https://github.com/josealmada/working_days_scaler.git
```

Build using cargo
```shell
cargo build --release
```

## Usage

```text
Usage: working_days_scaler.exe [OPTIONS]

Options:
      --holidays-file <HOLIDAYS_FILE>
          Path to the holidays CSV [default: holidays.csv]
  -p, --port <PORT>
          The port that the gRPC server will be listening [default: 8080]
  -t, --time-offset <TIME_OFFSET>
          The time offset in seconds. Value between -86400 and -86400 [default: 0]
  -h, --help
          Print help information
  -V, --version
          Print version information

```

KEDA ScaleObject
```yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: scaledobject-name
  namespace: scaledobject-namespace
spec:
  scaleTargetRef:
    name: deployment-name
  triggers:
    - type: external
      metadata:
        scalerAddress: working_days_scaler:8080
        nthWorkingDay: "5"
        fromTime: "06:00:00"
        toTime: "06:00:00"
        targetSize: "10"
```