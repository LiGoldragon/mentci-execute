use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::path::PathBuf;

pub mod root_guard;
pub mod link_guard;
pub mod program_version;
pub mod subject_unifier;
pub mod session_actor;
pub mod intent_actor;
pub mod session_guard;
pub mod report_actor;
pub mod launcher;
pub mod transition_actor;

#[derive(Debug)]
pub enum SymbolicMessage {
    ValidateRoot(RpcReplyPort<Result<(), Vec<String>>>),
    ValidateLinks(RpcReplyPort<Result<(), Vec<String>>>),
    ValidateSession(RpcReplyPort<Result<(), Vec<String>>>),
    GetProgramVersion(RpcReplyPort<String>),
    UnifySubjects(bool, RpcReplyPort<Result<(), String>>),
    InitializeIntent(String, RpcReplyPort<Result<String, String>>),
    LaunchJail(RpcReplyPort<Result<(), String>>),
    TransitionSession(RpcReplyPort<Result<(), String>>),
    EmitReport(
        String, // prompt
        String, // answer
        String, // subject
        String, // title
        String, // kind
        RpcReplyPort<Result<PathBuf, String>>,
    ),
    FinalizeSession(
        String, // summary
        String, // prompt
        String, // context
        Vec<String>, // changes
        String, // bookmark
        String, // remote
        String, // rev
        bool,   // no_push
        String, // model
        RpcReplyPort<Result<(), String>>,
    ),
}

pub struct Orchestrator;

#[async_trait::async_trait]
impl Actor for Orchestrator {
    type Msg = SymbolicMessage;
    type State = OrchestratorState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(OrchestratorState {})
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            SymbolicMessage::ValidateRoot(reply) => {
                let (actor, _handle) = Actor::spawn(None, root_guard::RootGuard, ()).await?;
                let res = ractor::call!(actor, root_guard::RootGuardMessage::Check)?;
                reply.send(res)?;
            }
            SymbolicMessage::ValidateLinks(reply) => {
                let (actor, _handle) = Actor::spawn(None, link_guard::LinkGuard, ()).await?;
                let res = ractor::call!(actor, link_guard::LinkGuardMessage::Check)?;
                reply.send(res)?;
            }
            SymbolicMessage::ValidateSession(reply) => {
                let (actor, _handle) = Actor::spawn(None, session_guard::SessionGuard, ()).await?;
                let res = ractor::call!(actor, session_guard::SessionGuardMessage::Check)?;
                reply.send(res)?;
            }
            SymbolicMessage::GetProgramVersion(reply) => {
                let (actor, _handle) = Actor::spawn(None, program_version::ProgramVersion, ()).await?;
                let res = ractor::call!(actor, program_version::ProgramVersionMessage::Get)?;
                reply.send(res)?;
            }
            SymbolicMessage::UnifySubjects(write, reply) => {
                let (actor, _handle) = Actor::spawn(None, subject_unifier::SubjectUnifier, ()).await?;
                let res = ractor::call!(actor, subject_unifier::SubjectUnifierMessage::Unify, write)?;
                reply.send(res)?;
            }
            SymbolicMessage::InitializeIntent(name, reply) => {
                let (actor, _handle) = Actor::spawn(None, intent_actor::IntentActor, ()).await?;
                let res = ractor::call!(actor, intent_actor::IntentMessage::Initialize, name)?;
                reply.send(res)?;
            }
            SymbolicMessage::LaunchJail(reply) => {
                let (actor, _handle) = Actor::spawn(None, launcher::Launcher, ()).await?;
                let res = ractor::call!(actor, launcher::LauncherMessage::Launch)?;
                reply.send(res)?;
            }
            SymbolicMessage::TransitionSession(reply) => {
                let (actor, _handle) = Actor::spawn(None, transition_actor::TransitionActor, ()).await?;
                let res = ractor::call!(actor, transition_actor::TransitionMessage::Run)?;
                reply.send(res)?;
            }
            SymbolicMessage::EmitReport(prompt, answer, subject, title, kind, reply) => {
                let (actor, _handle) = Actor::spawn(None, report_actor::ReportActor, ()).await?;
                let res = ractor::call!(actor, report_actor::ReportMessage::Emit, prompt, answer, subject, title, kind)?;
                reply.send(res)?;
            }
            SymbolicMessage::FinalizeSession(summary, prompt, context, changes, bookmark, remote, rev, no_push, model, reply) => {
                let (actor, _handle) = Actor::spawn(None, session_actor::SessionActor, ()).await?;
                let res = ractor::call!(actor, session_actor::SessionMessage::Finalize, summary, prompt, context, changes, bookmark, remote, rev, no_push, model)?;
                reply.send(res)?;
            }
        }
        Ok(())
    }
}

pub struct OrchestratorState {}
