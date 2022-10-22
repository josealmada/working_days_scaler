use chrono::Utc;
use tonic::{Request, Response, Status};

use crate::WorkingDays;

tonic::include_proto!("externalscaler");

#[derive(Debug)]
pub struct GrpcHandler {
    pub working_days: WorkingDays,
}

#[tonic::async_trait]
impl external_scaler_server::ExternalScaler for GrpcHandler {
    async fn is_active(
        &self,
        request: Request<ScaledObjectRef>,
    ) -> Result<Response<IsActiveResponse>, Status> {
        let message = request.into_inner();
        let expected_nth_working_day: u8 = read_nth_working_day_arg(&message)?;
        let nth_working_day = current_nth_working_day(&self.working_days)?;

        Ok(Response::new(IsActiveResponse {
            result: expected_nth_working_day == nth_working_day,
        }))
    }

    async fn get_metric_spec(
        &self,
        request: Request<ScaledObjectRef>,
    ) -> Result<Response<GetMetricSpecResponse>, Status> {
        let message = request.into_inner();
        let target_size = read_target_size(&message)?;

        Ok(Response::new(GetMetricSpecResponse {
            metric_specs: vec![MetricSpec {
                metric_name: "nth_working_day".to_string(),
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

fn read_nth_working_day_arg(message: &ScaledObjectRef) -> Result<u8, Status> {
    let value = message.scaler_metadata.get("nth_working_day");
    match value {
        None => Err(Status::invalid_argument(
            "Missing required metadata `nth_working_day`.",
        )),
        Some(value) => {
            if let Ok(parsed) = value.parse::<u8>() {
                if parsed <= 31 {
                    Ok(parsed)
                } else {
                    Err(Status::invalid_argument(
                        "Metadata `nth_working_day` should be a value between 1 and 31.",
                    ))
                }
            } else {
                Err(Status::invalid_argument(
                    "Metadata `nth_working_day` should be a value between 1 and 31.",
                ))
            }
        }
    }
}

fn read_target_size(message: &ScaledObjectRef) -> Result<u32, Status> {
    let value = message.scaler_metadata.get("target_size");
    match value {
        None => Err(Status::invalid_argument(
            "Missing required metadata `target_size`.",
        )),
        Some(value) => {
            if let Ok(parsed) = value.parse::<u32>() {
                Ok(parsed)
            } else {
                Err(Status::invalid_argument(
                    "Metadata `target_size` should be an integer value.",
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{FixedOffset, TimeZone};
    use tonic::Request;

    use crate::handler::external_scaler_server::ExternalScaler;
    use crate::handler::{GetMetricsRequest, ScaledObjectRef};
    use crate::{GrpcHandler, WorkingDays};

    #[tokio::test]
    async fn should_require_valid_nth_working_day_argument() {
        let handler = GrpcHandler {
            working_days: simple_working_days(),
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
            "Missing required metadata `nth_working_day`."
        );

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nth_working_day".to_string(), "jose".to_string());

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
            "Metadata `nth_working_day` should be a value between 1 and 31."
        );

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nth_working_day".to_string(), "32".to_string());

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
            "Metadata `nth_working_day` should be a value between 1 and 31."
        );
    }

    #[tokio::test]
    async fn should_require_valid_target_size_argument() {
        let handler = GrpcHandler {
            working_days: simple_working_days(),
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
            "Missing required metadata `target_size`."
        );

        let handler = GrpcHandler {
            working_days: simple_working_days(),
        };

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("target_size".to_string(), "jose".to_string());

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
            "Metadata `target_size` should be an integer value."
        );
    }

    #[tokio::test]
    async fn should_return_error_if_today_is_out_of_range() {
        let handler = GrpcHandler {
            working_days: out_of_range_working_days(),
        };

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nth_working_day".to_string(), "5".to_string());

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
        };

        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("nth_working_day".to_string(), "5".to_string());
        metadata.insert("target_size".to_string(), "10".to_string());

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

    fn simple_working_days() -> WorkingDays {
        let mut holidays = Vec::new();
        let offset = FixedOffset::west(3 * 3600);

        holidays.push(offset.ymd(2022, 6, 5));
        holidays.push(offset.ymd(2122, 6, 5));

        WorkingDays::build(offset, holidays).unwrap()
    }

    fn out_of_range_working_days() -> WorkingDays {
        let mut holidays = Vec::new();
        let offset = FixedOffset::west(3 * 3600);

        holidays.push(offset.ymd(2020, 6, 5));
        holidays.push(offset.ymd(2021, 6, 5));

        WorkingDays::build(offset, holidays).unwrap()
    }
}
