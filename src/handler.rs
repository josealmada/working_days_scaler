use std::sync::Arc;
use std::time::Duration;

use chrono::{NaiveTime, Utc};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::WorkingDays;

tonic::include_proto!("externalscaler");

#[derive(Debug)]
pub struct GrpcHandler {
    pub working_days: Arc<WorkingDays>,
    pub push_interval: u64,
}

#[tonic::async_trait]
impl external_scaler_server::ExternalScaler for GrpcHandler {
    async fn is_active(
        &self,
        request: Request<ScaledObjectRef>,
    ) -> Result<Response<IsActiveResponse>, Status> {
        is_active(&self.working_days, request.into_inner())
            .await
            .map(Response::new)
    }

    type StreamIsActiveStream = ReceiverStream<Result<IsActiveResponse, Status>>;

    async fn stream_is_active(
        &self,
        request: Request<ScaledObjectRef>,
    ) -> Result<Response<Self::StreamIsActiveStream>, Status> {
        let (tx, rx) = mpsc::channel(1);

        let working_days = Arc::clone(&self.working_days);
        let push_interval = Duration::from_secs(self.push_interval);
        let message = request.into_inner();
        tokio::spawn(async move {
            while !tx.is_closed() {
                let result = is_active(&working_days, message.clone()).await;

                if (tx.send(result).await).is_ok() {
                    tokio::time::sleep(push_interval).await
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn get_metric_spec(
        &self,
        request: Request<ScaledObjectRef>,
    ) -> Result<Response<GetMetricSpecResponse>, Status> {
        let message = request.into_inner();
        let target_size = read_target_size(&message)?;

        Ok(Response::new(GetMetricSpecResponse {
            metric_specs: vec![MetricSpec {
                metric_name: "nthWorkingDay".to_string(),
                target_size: target_size as i64,
            }],
        }))
    }

    async fn get_metrics(
        &self,
        request: Request<GetMetricsRequest>,
    ) -> Result<Response<GetMetricsResponse>, Status> {
        let nth_working_day = current_nth_working_day(&self.working_days)?;

        Ok(Response::new(GetMetricsResponse {
            metric_values: vec![MetricValue {
                metric_name: request.into_inner().metric_name,
                metric_value: nth_working_day as i64,
            }],
        }))
    }
}

async fn is_active(
    working_days: &WorkingDays,
    message: ScaledObjectRef,
) -> Result<IsActiveResponse, Status> {
    let expected_nth_working_day: u8 = read_nth_working_day_arg(&message)?;
    let from_time = read_time(&message, "fromTime")?;
    let to_time = read_time(&message, "toTime")?;

    read_target_size(&message)?; // Checking if present to avoid later errors

    let nth_working_day = current_nth_working_day(working_days)?;

    Ok(IsActiveResponse {
        result: expected_nth_working_day == nth_working_day
            && current_time_between(working_days, from_time, to_time),
    })
}

fn read_nth_working_day_arg(message: &ScaledObjectRef) -> Result<u8, Status> {
    let value = message.scaler_metadata.get("nthWorkingDay");
    match value {
        None => Err(Status::invalid_argument(
            "Missing required metadata `nthWorkingDay`.",
        )),
        Some(value) => {
            if let Ok(parsed) = value.parse::<u8>() {
                if parsed <= 31 {
                    Ok(parsed)
                } else {
                    Err(Status::invalid_argument(
                        "Metadata `nthWorkingDay` should be a value between 1 and 31.",
                    ))
                }
            } else {
                Err(Status::invalid_argument(
                    "Metadata `nthWorkingDay` should be a value between 1 and 31.",
                ))
            }
        }
    }
}

fn read_time(message: &ScaledObjectRef, parameter: &str) -> Result<NaiveTime, Status> {
    let value = message.scaler_metadata.get(parameter);
    match value {
        None => Err(Status::invalid_argument(format!(
            "Missing required metadata `{}`.",
            parameter
        ))),
        Some(value) => {
            if let Ok(parsed) = NaiveTime::parse_from_str(value, "%H:%M:%S") {
                Ok(parsed)
            } else {
                Err(Status::invalid_argument(format!(
                    "Metadata `{}` should be an time formatted as `%H:%M:%S`.",
                    parameter
                )))
            }
        }
    }
}

fn read_target_size(message: &ScaledObjectRef) -> Result<u32, Status> {
    let value = message.scaler_metadata.get("targetSize");
    match value {
        None => Err(Status::invalid_argument(
            "Missing required metadata `targetSize`.",
        )),
        Some(value) => {
            if let Ok(parsed) = value.parse::<u32>() {
                Ok(parsed)
            } else {
                Err(Status::invalid_argument(
                    "Metadata `targetSize` should be an integer value.",
                ))
            }
        }
    }
}

fn current_nth_working_day(working_days: &WorkingDays) -> Result<u8, Status> {
    let now = Utc::now().with_timezone(&working_days.time_offset);
    let result = working_days.working_days_mtd(now.date());
    result.map_err(|err| Status::invalid_argument(err.to_string()))
}

fn current_time_between(working_days: &WorkingDays, from: NaiveTime, to: NaiveTime) -> bool {
    let time = Utc::now().with_timezone(&working_days.time_offset).time();

    from <= time && time <= to
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use chrono::{FixedOffset, TimeZone};
    use tonic::Request;

    use crate::handler::external_scaler_server::ExternalScaler;
    use crate::handler::{GetMetricsRequest, ScaledObjectRef};
    use crate::{GrpcHandler, WorkingDays};

    #[tokio::test]
    async fn should_require_valid_nth_working_day_argument() {
        let handler = GrpcHandler {
            working_days: simple_working_days(),
            push_interval: 60,
        };

        let result = handler
            .is_active(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: HashMap::new(),
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message().to_string(),
            "Missing required metadata `nthWorkingDay`."
        );

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nthWorkingDay".to_string(), "jose".to_string());

        let result = handler
            .is_active(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata,
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message().to_string(),
            "Metadata `nthWorkingDay` should be a value between 1 and 31."
        );

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nthWorkingDay".to_string(), "32".to_string());

        let result = handler
            .is_active(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata,
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message().to_string(),
            "Metadata `nthWorkingDay` should be a value between 1 and 31."
        );
    }

    #[tokio::test]
    async fn should_require_valid_target_size_argument() {
        let handler = GrpcHandler {
            working_days: simple_working_days(),
            push_interval: 60,
        };

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nthWorkingDay".to_string(), "5".to_string());
        metadata.insert("fromTime".to_string(), "06:00:00".to_string());
        metadata.insert("toTime".to_string(), "18:00:00".to_string());

        let result = handler
            .is_active(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata,
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message().to_string(),
            "Missing required metadata `targetSize`."
        );

        let handler = GrpcHandler {
            working_days: simple_working_days(),
            push_interval: 60,
        };

        let result = handler
            .get_metric_spec(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: HashMap::new(),
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message().to_string(),
            "Missing required metadata `targetSize`."
        );

        let handler = GrpcHandler {
            working_days: simple_working_days(),
            push_interval: 60,
        };

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("targetSize".to_string(), "jose".to_string());

        let result = handler
            .get_metric_spec(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata,
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message().to_string(),
            "Metadata `targetSize` should be an integer value."
        );
    }

    #[tokio::test]
    async fn should_require_valid_from_date_and_to_date() {
        let handler = GrpcHandler {
            working_days: simple_working_days(),
            push_interval: 60,
        };

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nthWorkingDay".to_string(), "5".to_string());

        let result = handler
            .is_active(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata,
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message().to_string(),
            "Missing required metadata `fromTime`."
        );

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nthWorkingDay".to_string(), "5".to_string());
        metadata.insert("fromTime".to_string(), "06:00:00".to_string());

        let result = handler
            .is_active(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata,
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message().to_string(),
            "Missing required metadata `toTime`."
        );

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nthWorkingDay".to_string(), "5".to_string());
        metadata.insert("fromTime".to_string(), "06:00:00".to_string());
        metadata.insert("toTime".to_string(), "00:00".to_string());

        let result = handler
            .is_active(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata,
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message().to_string(),
            "Metadata `toTime` should be an time formatted as `%H:%M:%S`."
        );
    }

    #[tokio::test]
    async fn should_return_error_if_today_is_out_of_range() {
        let handler = GrpcHandler {
            working_days: out_of_range_working_days(),
            push_interval: 60,
        };

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nthWorkingDay".to_string(), "5".to_string());
        metadata.insert("fromTime".to_string(), "06:00:00".to_string());
        metadata.insert("toTime".to_string(), "18:00:00".to_string());
        metadata.insert("targetSize".to_string(), "10".to_string());

        let result = handler
            .is_active(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata,
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().message().to_string(), "The requested date was not calculated. Table processed for dates between 2020-01-01-03:00 and 2021-12-31-03:00.");

        let result = handler
            .get_metrics(Request::new(GetMetricsRequest {
                scaled_object_ref: None,
                metric_name: "metric_name".to_string(),
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().message().to_string(), "The requested date was not calculated. Table processed for dates between 2020-01-01-03:00 and 2021-12-31-03:00.");
    }

    #[tokio::test]
    async fn should_execute_without_errors() {
        let handler = GrpcHandler {
            working_days: simple_working_days(),
            push_interval: 60,
        };

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nthWorkingDay".to_string(), "5".to_string());
        metadata.insert("fromTime".to_string(), "06:00:00".to_string());
        metadata.insert("toTime".to_string(), "18:00:00".to_string());
        metadata.insert("targetSize".to_string(), "10".to_string());

        let result = handler
            .is_active(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata.clone(),
            }))
            .await;

        assert!(result.is_ok());

        let result = handler
            .get_metric_spec(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata,
            }))
            .await;

        assert!(result.is_ok());

        let result = handler
            .get_metrics(Request::new(GetMetricsRequest {
                scaled_object_ref: None,
                metric_name: "metric_name".to_string(),
            }))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_execute_stream_without_errors() {
        let handler = GrpcHandler {
            working_days: simple_working_days(),
            push_interval: 1,
        };

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nthWorkingDay".to_string(), "5".to_string());
        metadata.insert("fromTime".to_string(), "06:00:00".to_string());
        metadata.insert("toTime".to_string(), "18:00:00".to_string());
        metadata.insert("targetSize".to_string(), "10".to_string());

        let result = handler
            .stream_is_active(Request::new(ScaledObjectRef {
                name: "name".to_string(),
                namespace: "namespace".to_string(),
                scaler_metadata: metadata.clone(),
            }))
            .await;

        assert!(result.is_ok());

        let mut stream = result.unwrap().into_inner().into_inner();

        assert!(stream.recv().await.unwrap().is_ok());
        assert!(stream.recv().await.unwrap().is_ok());
        assert!(stream.recv().await.unwrap().is_ok());
        drop(stream);
    }

    fn simple_working_days() -> Arc<WorkingDays> {
        let mut holidays = Vec::new();
        let offset = FixedOffset::west(3 * 3600);

        holidays.push(offset.ymd(2022, 6, 5));
        holidays.push(offset.ymd(2122, 6, 5));

        Arc::new(WorkingDays::build(offset, holidays).unwrap())
    }

    fn out_of_range_working_days() -> Arc<WorkingDays> {
        let mut holidays = Vec::new();
        let offset = FixedOffset::west(3 * 3600);

        holidays.push(offset.ymd(2020, 6, 5));
        holidays.push(offset.ymd(2021, 6, 5));

        Arc::new(WorkingDays::build(offset, holidays).unwrap())
    }
}
