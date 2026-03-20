pub mod envelope;
pub mod events;
pub mod inbound;
pub mod registry;
pub mod reply;
pub mod routing;
pub mod sender;

#[allow(unused_imports)]
pub use envelope::{Attachment, MessageEnvelope, RoutingMeta};
pub use events::{MessageReceivedEvent, MessageSentEvent};
#[allow(unused_imports)]
pub use inbound::{
    ChannelInfo, ChatInfo, CommandInfo, ContentVariants, ForwardInfo, InboundMessage, MediaInfo,
    MediaUnderstanding, MessageInfo, ReplyInfo, SenderInfo, StickerInfo, ThreadInfo,
};
pub use registry::ChannelRegistry;
pub use reply::AgentReply;
#[allow(unused_imports)]
pub use routing::{
    resolve_delivery_target, update_last_route, RouteError, SessionDeliveryState, TurnSource,
};
#[allow(unused_imports)]
pub use sender::{ChannelSender, SendResult};
