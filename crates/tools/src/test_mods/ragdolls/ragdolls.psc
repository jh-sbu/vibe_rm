Scriptname VrmRagdollsScript extends Quest
GlobalVariable Property Run Auto
ActorBase Property Base Auto
ActorBase Property ImmuneBase Auto
ActorBase Property MammothBase Auto
ActorBase Property CenturionBase Auto
Spell Property Probe Auto
Spell Property Paralyze1 Auto
Spell Property Paralyze5 Auto
MagicEffect Property State0 Auto
MagicEffect Property State1 Auto
MagicEffect Property State2 Auto
MagicEffect Property State3 Auto
MagicEffect Property State4 Auto
MagicEffect Property State5 Auto
MagicEffect Property State6 Auto
MagicEffect Property State7 Auto
MagicEffect Property State8 Auto
MagicEffect Property Legacy0 Auto
MagicEffect Property Legacy1 Auto
MagicEffect Property Legacy2 Auto
MagicEffect Property Legacy3 Auto
MagicEffect Property Legacy4 Auto
MagicEffect Property Legacy5 Auto
MagicEffect Property Legacy6 Auto
MagicEffect Property Legacy7 Auto
MagicEffect Property Legacy8 Auto
Actor Player
Actor Target
Int Phase = 0
Int Samples = 0
Int EventCount = 0
Float Epoch = 0.0
Bool Watching = false
String ResultSetup
String ResultPush5
String ResultPush10
String ResultPush15
String ResultPull10
String ResultArea025
String ResultArea050
String ResultArea075
String ResultArea099
String ResultArea100
String ResultArea150
String ResultImmunity
String ResultParalysis1
String ResultParalysis5
String ResultConstraints
String ResultForceAdd
String ResultForceRemove
String ResultDeath
String ResultReload
String ResultWeapons
Int Rows = 0
Int Page = 1
String ResultPage1
String ResultPage2
String ResultPage3
String ResultPage4
String ResultPage5
String ResultPage6
String ResultPage7
String ResultPage8
String ResultPage9
String ResultPage10
String ResultPage11
String ResultPage12
String ResultPage13
String ResultPage14
String ResultPage15
String ResultPage16
String ResultPage17
String ResultPage18
String ResultPage19
String ResultPage20
String ResultPage21
String ResultPage22
String ResultPage23
String ResultPage24
String ResultPage25
String ResultPage26
String ResultPage27
String ResultPage28
String ResultPage29
String ResultPage30
String ResultPage31
String ResultPage32
String ResultPage33
String ResultPage34
String ResultPage35
String ResultPage36
String ResultPage37
String ResultPage38
String ResultPage39
String ResultPage40
String ResultPage41
String ResultPage42
String ResultPage43
String ResultPage44
String ResultPage45
String ResultPage46
String ResultPage47
String ResultPage48
String ResultPage49
String ResultPage50
String ResultPage51
String ResultPage52
String ResultPage53
String ResultPage54
String ResultPage55
String ResultPage56
String ResultPage57
String ResultPage58
String ResultPage59
String ResultPage60
String ResultPage61
String ResultPage62
String ResultPage63
String ResultPage64
String ResultPage65
String ResultPage66
String ResultPage67
String ResultPage68
String ResultPage69
String ResultPage70
String ResultPage71
String ResultPage72
String ResultPage73
String ResultPage74
String ResultPage75
String ResultPage76
String ResultPage77
String ResultPage78
String ResultPage79
String ResultPage80
String ResultPage81
String ResultPage82
String ResultPage83
String ResultPage84
String ResultPage85
String ResultPage86
String ResultPage87
String ResultPage88
String ResultPage89
String ResultPage90
String ResultPage91
String ResultPage92
String ResultPage93
String ResultPage94
String ResultPage95
String ResultPage96

Event OnInit()
    RegisterForSingleUpdate(4.0)
EndEvent

Event OnUpdate()
    Player = Game.GetPlayer()
    Int next = Run.GetValueInt()
    If next != Phase
        Watching = false
        Phase = next
        Samples = 0
        EventCount = 0
        Epoch = Utility.GetCurrentRealTime()
        If Rows > 0
            Page += 1
            Rows = 0
        EndIf
        AddLine("phase " + Phase)
        If Phase == 0
        ElseIf Phase == 20
            Game.RequestSave()
            Debug.MessageBox("Ragdoll probe saved. Read with vrm-tool test-results. Please also report whether weapons stayed held, front/back get-up looked correct, and the body's pose on death. No results are inferred from this script alone.")
        ElseIf Phase == 18
            MakeTarget(false)
            Player.PushActorAway(Target, 10.0)
            Utility.Wait(0.25)
            ResultReload = "at save request state " + ReadState() + " position " + Position()
            Game.RequestSave()
            Debug.MessageBox("A save was requested during this actor's fall. Reload it, then set VrmRagRun to 19. For the separate unload test, repeat 18, leave the cell promptly while the actor is down, return, then set to 19. Report which test each save is.")
        ElseIf Phase == 19
            ResultReload = "after reload state " + ReadState() + " position " + Position()
            ResultWeapons = "drawn=" + Target.IsWeaponDrawn()
        ElseIf Phase == 16
            Target.ForceRemoveRagdollFromWorld()
            Watching = true
        ElseIf Phase == 17
            Player.PushActorAway(Target, 10.0)
            Utility.Wait(0.5)
            Target.Kill(Player)
            ResultDeath = "dead=" + Target.IsDead() + " position " + Position()
            Watching = true
        Else
            MakeTarget(Phase == 11)
            Watching = true
            If Phase == 1
                Player.PushActorAway(Target, 5.0)
            ElseIf Phase == 2
                Player.PushActorAway(Target, 10.0)
            ElseIf Phase == 3 || Phase == 11 || Phase == 21 || Phase == 22
                Player.PushActorAway(Target, 15.0)
            ElseIf Phase == 4
                Player.PushActorAway(Target, -10.0)
            ElseIf Phase >= 5 && Phase <= 10
                Float mag = 0.25
                If Phase == 6
                    mag = 0.5
                ElseIf Phase == 7
                    mag = 0.75
                ElseIf Phase == 8
                    mag = 0.99
                ElseIf Phase == 9
                    mag = 1.0
                ElseIf Phase == 10
                    mag = 1.5
                EndIf
                Player.KnockAreaEffect(mag, 800.0)
                AddLine("area magnitude " + mag + " radius 800 distance " + Target.GetDistance(Player))
            ElseIf Phase == 12
                Paralyze1.Cast(Player, Target)
            ElseIf Phase == 13
                Paralyze5.Cast(Player, Target)
            ElseIf Phase == 14
                ResultConstraints = "standing=" + Game.AddHavokBallAndSocketConstraint(Target, "NPC L Hand [LHnd]", Player, "NPC L Hand [LHnd]", 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                Game.RemoveHavokConstraints(Target, "NPC L Hand [LHnd]", Player, "NPC L Hand [LHnd]")
                Player.PushActorAway(Target, 10.0)
                Utility.Wait(0.5)
                ResultConstraints += " down=" + Game.AddHavokBallAndSocketConstraint(Target, "NPC L Hand [LHnd]", Player, "NPC L Hand [LHnd]", 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            ElseIf Phase == 15
                Target.ForceAddRagdollToWorld()
            EndIf
        EndIf
    EndIf
    If Watching
        AddLine("t=" + (Utility.GetCurrentRealTime() - Epoch) + " " + Position() + " enum/legacy=" + ReadState() + " drawn=" + Target.IsWeaponDrawn())
        Samples += 1
        If Samples >= 80
            Watching = false
            If Phase == 14
                ResultConstraints += " after=" + Game.RemoveHavokConstraints(Target, "NPC L Hand [LHnd]", Player, "NPC L Hand [LHnd]")
            EndIf
            Debug.Notification("Ragdoll phase " + Phase + " watch timed out. Set VrmRagRun to the next phase; 20 saves.")
        EndIf
    ElseIf Phase == 0 && ResultSetup == ""
        ResultSetup = "No SKSE. enum and legacy states are measured by conditioned scripted-archetype effects with no script or actor-value changes; spell dispatch adds sampling latency. Actor AI is disabled to keep ground displacement measurable. Event timestamps use real seconds."
        Debug.MessageBox("Ragdoll probe: throwaway save, flat open ground. set VrmRagRun to 1..4: pushes 5/10/15/-10; 5..10: areas .25/.5/.75/.99/1/1.5; 11: flame atronach immunity; 12/13: paralysis 1/5 sec; 14: constraints; 15: force add; 16: force remove; 17: death while down; 18/19: manual reload; 20: save reports; 21/22: mammoth/centurion immunity. Keep each phase's actor in view. Each page has at most 20 lines; sampling continues until recovery or 80 samples. Do not use cqf.")
    EndIf
    RegisterForSingleUpdate(0.25)
EndEvent

Function MakeTarget(Bool immune)
    If Target
        Target.Disable()
        Target.Delete()
    EndIf
    If Phase == 21
        Target = Player.PlaceAtMe(MammothBase, 1, false, false) as Actor
    ElseIf Phase == 22
        Target = Player.PlaceAtMe(CenturionBase, 1, false, false) as Actor
    ElseIf immune
        Target = Player.PlaceAtMe(ImmuneBase, 1, false, false) as Actor
    Else
        Target = Player.PlaceAtMe(Base, 1, false, false) as Actor
    EndIf
    Target.MoveTo(Player, 0.0, 200.0, 0.0, false)
    Target.EnableAI(false)
    Target.DrawWeapon()
    RegisterForAnimationEvent(Target, "AddCharacterControllerToWorld")
    RegisterForAnimationEvent(Target, "GetUpEnd")
    RegisterForAnimationEvent(Target, "Getup")
    RegisterForAnimationEvent(Target, "GetUpExit")
EndFunction

Event OnAnimationEvent(ObjectReference source, String eventName)
    If source == Target && EventCount < 18
        AddLine("event t=" + (Utility.GetCurrentRealTime() - Epoch) + " " + eventName)
        If eventName == "GetUpEnd" || eventName == "GetUpExit"
            Watching = false
            If Phase == 14
                Utility.Wait(0.05)
                ResultConstraints += " after=" + Game.RemoveHavokConstraints(Target, "NPC L Hand [LHnd]", Player, "NPC L Hand [LHnd]")
            EndIf
        EndIf
        EventCount += 1
    EndIf
EndEvent

String Function Position()
    Return "xyz=" + Target.GetPositionX() + "," + Target.GetPositionY() + "," + Target.GetPositionZ()
EndFunction

String Function ReadState()
    If !Target
        Return "none"
    EndIf
    Target.DispelSpell(Probe)
    Probe.Cast(Target, Target)
    Utility.Wait(0.02)
    Int state = -1
    Int legacy = -1
    If Target.HasMagicEffect(State0)
        state = 0
    ElseIf Target.HasMagicEffect(State1)
        state = 1
    ElseIf Target.HasMagicEffect(State2)
        state = 2
    ElseIf Target.HasMagicEffect(State3)
        state = 3
    ElseIf Target.HasMagicEffect(State4)
        state = 4
    ElseIf Target.HasMagicEffect(State5)
        state = 5
    ElseIf Target.HasMagicEffect(State6)
        state = 6
    ElseIf Target.HasMagicEffect(State7)
        state = 7
    ElseIf Target.HasMagicEffect(State8)
        state = 8
    EndIf
    If Target.HasMagicEffect(Legacy0)
        legacy = 0
    ElseIf Target.HasMagicEffect(Legacy1)
        legacy = 1
    ElseIf Target.HasMagicEffect(Legacy2)
        legacy = 2
    ElseIf Target.HasMagicEffect(Legacy3)
        legacy = 3
    ElseIf Target.HasMagicEffect(Legacy4)
        legacy = 4
    ElseIf Target.HasMagicEffect(Legacy5)
        legacy = 5
    ElseIf Target.HasMagicEffect(Legacy6)
        legacy = 6
    ElseIf Target.HasMagicEffect(Legacy7)
        legacy = 7
    ElseIf Target.HasMagicEffect(Legacy8)
        legacy = 8
    EndIf
    Return state + "/" + legacy
EndFunction

Function AddLine(String line)
    If Rows >= 20
        Rows = 0
        Page += 1
    EndIf
    Rows += 1
    line += "\n"
    If Page == 1
        ResultPage1 += line
    ElseIf Page == 2
        ResultPage2 += line
    ElseIf Page == 3
        ResultPage3 += line
    ElseIf Page == 4
        ResultPage4 += line
    ElseIf Page == 5
        ResultPage5 += line
    ElseIf Page == 6
        ResultPage6 += line
    ElseIf Page == 7
        ResultPage7 += line
    ElseIf Page == 8
        ResultPage8 += line
    ElseIf Page == 9
        ResultPage9 += line
    ElseIf Page == 10
        ResultPage10 += line
    ElseIf Page == 11
        ResultPage11 += line
    ElseIf Page == 12
        ResultPage12 += line
    ElseIf Page == 13
        ResultPage13 += line
    ElseIf Page == 14
        ResultPage14 += line
    ElseIf Page == 15
        ResultPage15 += line
    ElseIf Page == 16
        ResultPage16 += line
    ElseIf Page == 17
        ResultPage17 += line
    ElseIf Page == 18
        ResultPage18 += line
    ElseIf Page == 19
        ResultPage19 += line
    ElseIf Page == 20
        ResultPage20 += line
    ElseIf Page == 21
        ResultPage21 += line
    ElseIf Page == 22
        ResultPage22 += line
    ElseIf Page == 23
        ResultPage23 += line
    ElseIf Page == 24
        ResultPage24 += line
    ElseIf Page == 25
        ResultPage25 += line
    ElseIf Page == 26
        ResultPage26 += line
    ElseIf Page == 27
        ResultPage27 += line
    ElseIf Page == 28
        ResultPage28 += line
    ElseIf Page == 29
        ResultPage29 += line
    ElseIf Page == 30
        ResultPage30 += line
    ElseIf Page == 31
        ResultPage31 += line
    ElseIf Page == 32
        ResultPage32 += line
    ElseIf Page == 33
        ResultPage33 += line
    ElseIf Page == 34
        ResultPage34 += line
    ElseIf Page == 35
        ResultPage35 += line
    ElseIf Page == 36
        ResultPage36 += line
    ElseIf Page == 37
        ResultPage37 += line
    ElseIf Page == 38
        ResultPage38 += line
    ElseIf Page == 39
        ResultPage39 += line
    ElseIf Page == 40
        ResultPage40 += line
    ElseIf Page == 41
        ResultPage41 += line
    ElseIf Page == 42
        ResultPage42 += line
    ElseIf Page == 43
        ResultPage43 += line
    ElseIf Page == 44
        ResultPage44 += line
    ElseIf Page == 45
        ResultPage45 += line
    ElseIf Page == 46
        ResultPage46 += line
    ElseIf Page == 47
        ResultPage47 += line
    ElseIf Page == 48
        ResultPage48 += line
    ElseIf Page == 49
        ResultPage49 += line
    ElseIf Page == 50
        ResultPage50 += line
    ElseIf Page == 51
        ResultPage51 += line
    ElseIf Page == 52
        ResultPage52 += line
    ElseIf Page == 53
        ResultPage53 += line
    ElseIf Page == 54
        ResultPage54 += line
    ElseIf Page == 55
        ResultPage55 += line
    ElseIf Page == 56
        ResultPage56 += line
    ElseIf Page == 57
        ResultPage57 += line
    ElseIf Page == 58
        ResultPage58 += line
    ElseIf Page == 59
        ResultPage59 += line
    ElseIf Page == 60
        ResultPage60 += line
    ElseIf Page == 61
        ResultPage61 += line
    ElseIf Page == 62
        ResultPage62 += line
    ElseIf Page == 63
        ResultPage63 += line
    ElseIf Page == 64
        ResultPage64 += line
    ElseIf Page == 65
        ResultPage65 += line
    ElseIf Page == 66
        ResultPage66 += line
    ElseIf Page == 67
        ResultPage67 += line
    ElseIf Page == 68
        ResultPage68 += line
    ElseIf Page == 69
        ResultPage69 += line
    ElseIf Page == 70
        ResultPage70 += line
    ElseIf Page == 71
        ResultPage71 += line
    ElseIf Page == 72
        ResultPage72 += line
    ElseIf Page == 73
        ResultPage73 += line
    ElseIf Page == 74
        ResultPage74 += line
    ElseIf Page == 75
        ResultPage75 += line
    ElseIf Page == 76
        ResultPage76 += line
    ElseIf Page == 77
        ResultPage77 += line
    ElseIf Page == 78
        ResultPage78 += line
    ElseIf Page == 79
        ResultPage79 += line
    ElseIf Page == 80
        ResultPage80 += line
    ElseIf Page == 81
        ResultPage81 += line
    ElseIf Page == 82
        ResultPage82 += line
    ElseIf Page == 83
        ResultPage83 += line
    ElseIf Page == 84
        ResultPage84 += line
    ElseIf Page == 85
        ResultPage85 += line
    ElseIf Page == 86
        ResultPage86 += line
    ElseIf Page == 87
        ResultPage87 += line
    ElseIf Page == 88
        ResultPage88 += line
    ElseIf Page == 89
        ResultPage89 += line
    ElseIf Page == 90
        ResultPage90 += line
    ElseIf Page == 91
        ResultPage91 += line
    ElseIf Page == 92
        ResultPage92 += line
    ElseIf Page == 93
        ResultPage93 += line
    ElseIf Page == 94
        ResultPage94 += line
    ElseIf Page == 95
        ResultPage95 += line
    ElseIf Page == 96
        ResultPage96 += line
    EndIf
EndFunction
