use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, HttpResponse,
};
use futures_util::future::{ok, LocalBoxFuture, Ready};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::{env, rc::Rc};
use actix_web::body::EitherBody;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub id: i64,
    pub email: String,
    pub firstname: String,
    pub lastname: String,
    pub exp: usize,
}

pub struct AuthMiddlewareFactory;

impl<S, B> Transform<S, ServiceRequest> for AuthMiddlewareFactory
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = AuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuthMiddleware {
            service: Rc::new(service),
        })
    }
}

pub struct AuthMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();

        Box::pin(async move {
            // Get Authorization header
            let token_opt = req
                .headers()
                .get("Authorization")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
                .map(|s| s.to_string());

            if token_opt.is_none() {
                let response = HttpResponse::Unauthorized()
                    .json(serde_json::json!({
                        "status": false,
                        "message": "Token is required"
                    }))
                    .map_into_right_body();

                return Ok(req.into_response(response));
            }

            let token = token_opt.unwrap();
            let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "supersecretkey".to_string());

            // Decode token
            let decoded = decode::<Claims>(
                &token,
                &DecodingKey::from_secret(secret.as_bytes()),
                &Validation::default(),
            );

            match decoded {
                Ok(token_data) => {
                    req.extensions_mut().insert(token_data.claims);
                    let res = service.call(req).await?;
                    Ok(res.map_into_left_body()) 
                }
                Err(err) => {
                    let response = HttpResponse::BadRequest()
                        .json(serde_json::json!({
                            "status": false,
                            "message": "Invalid token",
                            "error": err.to_string()
                        }))
                        .map_into_right_body();

                    Ok(req.into_response(response))
                }
            }
        })
    }
}
