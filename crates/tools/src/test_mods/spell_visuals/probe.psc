Scriptname VrmSpellVisualsProbe extends Quest
; test-mod spell-visuals: spell visuals by eye (plans/spell-visuals.md,
; Manual SE probe). spell_visuals.rs has the protocol.

Actor Property Player Auto
EffectShader Property Shader Auto
Actor Property Caster Auto
GlobalVariable Property Mode Auto
GlobalVariable Property Run Auto
GlobalVariable Property GameHour Auto
Spell Property Flames Auto
Spell Property Frostbite Auto
Spell Property Fireball Auto
ObjectReference Property SpotRock Auto
ObjectReference Property SpotAir Auto
ObjectReference Property SpotSide Auto
ObjectReference Property SpotAirSide Auto

Bool Ran = false

Event OnInit()
    RegisterForSingleUpdate(3.0)
EndEvent

; The first update sets the probe up. After it the console's
; `set VrmFxRun to <n>` runs a step (vanilla has no command to call a
; script function; vibe_rm's `cqf VrmFx <function>` calls them by name).
; Checked once a second.
Event OnUpdate()
    If !Ran
        Ran = true
        Setup()
    Else
        Int Request = Run.GetValueInt()
        If Request != 0
            Run.SetValue(0.0)
            Step(Request)
        EndIf
    EndIf
    RegisterForSingleUpdate(1.0)
EndEvent

Function Step(Int Request)
    If Request == 1
        Rock()
    ElseIf Request == 2
        Air()
    ElseIf Request == 3
        Side()
    ElseIf Request == 4
        AirSide()
    ElseIf Request == 5
        NpcFlames()
    ElseIf Request == 6
        NpcFlamesAir()
    ElseIf Request == 7
        NpcFrost()
    ElseIf Request == 8
        NpcFrostAir()
    ElseIf Request == 9
        NpcFireball()
    ElseIf Request == 10
        NpcStop()
    ElseIf Request == 11
        Night()
    ElseIf Request == 12
        StopVisual()
    ElseIf Request == 13
        Help()
    EndIf
EndFunction

Function Setup()
    Game.ForceThirdPerson()
    Player.MoveTo(SpotRock)
    Player.AddSpell(Flames, false)
    Player.AddSpell(Frostbite, false)
    Player.AddSpell(Fireball, false)
    Caster.AddSpell(Flames, false)
    Caster.AddSpell(Frostbite, false)
    Caster.AddSpell(Fireball, false)
    Player.SetActorValue("Magicka", 1000.0)
    Caster.SetActorValue("Magicka", 100000.0)
    Shader.Play(Player, -1.0)
    Help()
    Utility.Wait(3.0)
    Game.RequestSave()
EndFunction

Function Help()
    Debug.MessageBox("vibe_rm spell visuals: console set VrmFxRun to <n>\n0. Console fw 81a (clear weather) to match vibe_rm's captures.\n1. Oakflesh: note the sheen now. Reload the save this asks for,\n   note whether the sheen is still there, then 12 (stops it).\n2. Your Flames: 1 (at the rock), hold Flames on the rock 5 s;\n   screenshot at 1 s and at 5 s. 2 (open air), hold it there;\n   screenshot.\n3. Caster: 5 (Flames at the rock), then 3 (side view); watch\n   and screenshot. 6 (Flames into open air), then 4 (its side\n   view). Walk around it as you like.\n4. 7 / 8: Frostbite the same. 9: Fireball, catch the trail.\n5. 11 (night), then repeat 2 and 3 for Flames. 10 stops the caster.\nScreenshots go in tests/results/spell-visuals/, named by step.\n13 shows this again.")
EndFunction

Function StopVisual()
    Shader.Stop(Player)
EndFunction

Function Rock()
    Player.MoveTo(SpotRock)
EndFunction

Function Air()
    Player.MoveTo(SpotAir)
EndFunction

Function Side()
    Player.MoveTo(SpotSide)
EndFunction

Function AirSide()
    Player.MoveTo(SpotAirSide)
EndFunction

; The caster's package is chosen by Mode (VrmFxNpcMode): 0 holds its place.
Function Cast(Float Value)
    Mode.SetValue(Value)
    Caster.EvaluatePackage()
EndFunction

Function NpcStop()
    Cast(0.0)
EndFunction

Function NpcFlames()
    Cast(1.0)
EndFunction

Function NpcFlamesAir()
    Cast(2.0)
EndFunction

Function NpcFrost()
    Cast(3.0)
EndFunction

Function NpcFrostAir()
    Cast(4.0)
EndFunction

Function NpcFireball()
    Cast(5.0)
EndFunction

Function Night()
    GameHour.SetValue(23.0)
EndFunction
