use chrono::Utc;
use tonic::{Request, Response, Status};
use tonic::transport::{Error, Server};
use tracing::info;

use crate::{ExternalScalerServer, WorkingDays};

tonic::include_proto!("externalscaler");

#[derive(Debug)]
pub struct ExternalScaler {
    working_days: WorkingDays,
}

#[tonic::async_trait]
impl external_scaler_server::ExternalScaler for ExternalScaler {
    async fn is_active(&self, request: Request<ScaledObjectRef>) -> Result<Response<IsActiveResponse>, Status> {
        let message = request.into_inner();
        let value = message.scaler_metadata.get("nth_working_day");

        match value {
            Some(value) => {
                let expected_nth_working_day: u8 = value.parse().unwrap();
                let now = Utc::now().with_timezone(&self.working_days.time_offset);
                let nth_working_day = self.working_days.working_days_mtd(now.date()).unwrap();

                Ok(Response::new(IsActiveResponse {
                    result: expected_nth_working_day == nth_working_day
                }))
            }
            None => panic!("Tratar")
        }
    }

    async fn get_metric_spec(&self, request: Request<ScaledObjectRef>) -> Result<Response<GetMetricSpecResponse>, Status> {
        todo!()
    }

    async fn get_metrics(&self, request: Request<GetMetricsRequest>) -> Result<Response<GetMetricsResponse>, Status> {
        todo!()
    }
}

pub async fn start_server(port: u16, working_days: WorkingDays) -> Result<(), Error> {
    let addr = format!("[::1]:{}", port).parse().unwrap();
    let external_scaler = ExternalScaler { working_days };

    info!("GRPC server listening on {}", addr);

    let server = Server::builder()
        .add_service(ExternalScalerServer::new(external_scaler))
        .serve(addr);

    server.await
}
