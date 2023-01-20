use graphql_client::GraphQLQuery;
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema_wallet_read.graphql",
    query_path = "graphql/operations.graphql",
    response_derives = "Debug"
)]
pub struct RequestChallenge;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema_wallet_read.graphql",
    query_path = "graphql/operations.graphql",
    response_derives = "Debug"
)]
pub struct StartSession;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema_wallet_read.graphql",
    query_path = "graphql/operations.graphql",
    response_derives = "Debug"
)]
pub struct PrepareWalletSession;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema_wallet_read.graphql",
    query_path = "graphql/operations.graphql",
    response_derives = "Debug"
)]
pub struct UnlockWallet;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema_wallet_read.graphql",
    query_path = "graphql/operations.graphql",
    response_derives = "Debug"
)]
pub struct RefreshSession;

#[allow(non_camel_case_types)]
type timestamptz = u64;
#[allow(non_camel_case_types)]
type uuid = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema_wallet_read.graphql",
    query_path = "graphql/operations.graphql",
    response_derives = "Debug"
)]
pub struct GetBusinessOwner;
