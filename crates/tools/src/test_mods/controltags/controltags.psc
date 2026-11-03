Scriptname VrmControlTagsScript extends Quest
; test-mod controltags: what Skyrim SE shows for [Event] tags in text
; (plans/control-text-tags.md). Visual only: Papyrus can't read rendered
; text. controltags.rs has the protocol.

Message Property Box Auto
Message Property NoteA Auto
Message Property NoteB Auto
Message Property HelpA Auto
Message Property HelpB Auto
GlobalVariable Property Run Auto

Bool Logged = false

; The console's `set VrmControlTagsRun to 1` runs Probe, `to 2` Hint (vanilla
; has no command to call a script function). Checked once a second.
Event OnInit()
    RegisterForSingleUpdate(1.0)
EndEvent

Event OnUpdate()
    Int Request = Run.GetValueInt()
    If Request != 0
        Run.SetValue(0.0)
        If Request == 1
            Probe()
        ElseIf Request == 2
            Hint()
        EndIf
    EndIf
    RegisterForSingleUpdate(1.0)
EndEvent

; Probe: the box, two notifications, the objective
; (and, the first time, the log entry), then the two help messages.
Function Probe()
    Utility.Wait(5.0)
    Box.Show(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    Utility.Wait(2.0)
    NoteA.Show(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    Utility.Wait(3.0)
    NoteB.Show(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    Utility.Wait(8.0)
    SetObjectiveDisplayed(10, true, true)
    If !Logged
        Logged = true
        SetStage(10)
    EndIf
    Utility.Wait(8.0)
    ShowHelp(HelpA, "VrmControlTagsA", 15.0)
    Utility.Wait(17.0)
    ShowHelp(HelpB, "VrmControlTagsB", 15.0)
EndFunction

; Hint: help A for 90 seconds, to rebind under.
Function Hint()
    Utility.Wait(3.0)
    ShowHelp(HelpA, "VrmControlTagsA", 90.0)
EndFunction

; The reset lets each run show it again past its one-time limit.
Function ShowHelp(Message Help, String HelpEvent, Float Seconds)
    Message.ResetHelpMessage(HelpEvent)
    Help.ShowAsHelpMessage(HelpEvent, Seconds, 0.0, 1)
EndFunction
