module Main exposing (main)

import Browser exposing (..)
import Html exposing (..)
import Html.Attributes exposing (..)
import Html.Events exposing (..)
import Http
import Json.Decode as Decode exposing (Decoder)
import Json.Decode.Pipeline as JsonP
import Json.Encode as Encode
import List
import List.Extra as LExtra
import RemoteData exposing (WebData)


type ExpandCollapse
    = Expand
    | Collapse


subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.none


main : Program Flags Model Msg
main =
    Browser.document
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        }


type State
    = Default
    | TCAccept
    | CheckedExe
    | PatchedExe


type alias Flags =
    { rust_version : String
    , windows : String
    , current_date : String
    }


type alias Model =
    { flags : Flags
    , state : State
    , installer_options : WebData InstallerOption
    , list_installer_options : InstallerOption
    , hide_list : List String
    , locked_options : List String
    , select_list : List String
    , selected : String
    , docs : String
    , image : String
    , modal : Bool
    , modal_text : String
    , loading : Bool
    , sc4_location : String
    , sc4_location_option : String
    , tc : Bool
    }


type Msg
    = NoOp
    | ReceiveStructure (WebData InstallerOption)
    | ToggleExpand ExpandCollapse String
    | SelectDocs String
    | AddCheckOption ( String, String )
    | AddCheckOptionWithChildren ( String, String, List OptionNode )
    | AddRadioOption ( String, String, List OptionNode )
    | RemoveOption String
    | RemoveOptionWithChildren ( String, String, List OptionNode )
    | ReceiveDocs (WebData String)
    | ChangeLocationOption String
    | ChangeExePath String
    | CheckExePath
    | GotExePathStatus (WebData ExeResponse)
    | GotExePatchStatus (WebData PatchResponse)
    | HideModal
    | AcceptTC
    | PatchExe
    | ImageError


init : Flags -> ( Model, Cmd Msg )
init flags =
    ( { flags = flags
      , state = Default
      , loading = False
      , modal = False
      , modal_text = ""
      , installer_options = RemoteData.Loading
      , list_installer_options = InstallerOption []
      , locked_options = []
      , hide_list = []
      , select_list = []
      , selected = ""
      , docs = ""
      , image = "Network Addon Mod"
      , sc4_location = "C:/Program Files/Steam/steamapps/common/SimCity 4 Deluxe/Apps/SimCity 4.exe"
      , sc4_location_option = "Steam"
      , tc = False
      }
    , fetchStructure
    )


type RadioCheck
    = Radio
    | RadioChecked
    | RadioFolder
    | Checked
    | Unchecked
    | Locked
    | ParentLocked


fetchStructure : Cmd Msg
fetchStructure =
    Http.get
        { url = "/structure"
        , expect = Http.expectJson (RemoteData.fromResult >> ReceiveStructure) decodeInstallerOption
        }


fetchDocs : String -> Cmd Msg
fetchDocs id =
    Http.get
        { url = "docs/" ++ (String.replace "/" "%2F" id |> String.replace "top%2F" "")
        , expect = Http.expectString (RemoteData.fromResult >> ReceiveDocs)
        }


radioCheckFromString : Decoder RadioCheck
radioCheckFromString =
    Decode.string
        |> Decode.andThen
            (\s ->
                case s of
                    "Radio" ->
                        Decode.succeed Radio

                    "RadioChecked" ->
                        Decode.succeed RadioChecked

                    "RadioFolder" ->
                        Decode.succeed RadioFolder

                    "Checked" ->
                        Decode.succeed Checked

                    "Locked" ->
                        Decode.succeed Locked

                    "ParentLocked" ->
                        Decode.succeed ParentLocked

                    "Unchecked" ->
                        Decode.succeed Unchecked

                    _ ->
                        Decode.fail <| "Unknown RadioCheck Option"
            )



-- radioCheckToString : RadioCheck -> String
-- radioCheckToString rc =
--     case rc of
--         Radio ->
--             "Radio"
--         RadioChecked ->
--             "RadioChecked"
--         RadioFolder ->
--             "RadioFolder"
--         Checked ->
--             "Checked"
--         Locked ->
--             "Locked"
--         Unchecked ->
--             "Unchecked"
--         ParentLocked ->
--             "ParentLocked"


type alias OptionNode =
    { name : String
    , radio_check : RadioCheck
    , children : InstallerOption
    , depth : Int
    , parent : String
    }


type InstallerOption
    = InstallerOption (List OptionNode)



-- defInstallerOption : InstallerOption
-- defInstallerOption =
--     InstallerOption []


decodeOptionNode : Decoder OptionNode
decodeOptionNode =
    Decode.succeed OptionNode
        |> JsonP.required "name" Decode.string
        |> JsonP.required "radio_check" radioCheckFromString
        |> JsonP.optional "children" (Decode.lazy (\_ -> decodeInstallerOption)) (InstallerOption [])
        |> JsonP.required "depth" Decode.int
        |> JsonP.required "parent" Decode.string


decodeInstallerOption : Decoder InstallerOption
decodeInstallerOption =
    Decode.map InstallerOption <| Decode.list (Decode.lazy (\_ -> decodeOptionNode))


update : Msg -> Model -> ( Model, Cmd Msg )
update message model =
    case message of
        NoOp ->
            ( model, Cmd.none )

        ToggleExpand expand_collapse id ->
            case expand_collapse of
                Expand ->
                    ( { model | hide_list = LExtra.remove id model.hide_list }, Cmd.none )

                Collapse ->
                    ( { model | hide_list = id :: model.hide_list }, Cmd.none )

        HideModal ->
            ( { model | modal = False }, Cmd.none )

        AcceptTC ->
            ( { model
                | tc = True
                , state =
                    if model.flags.windows == "true" then
                        TCAccept

                    else
                        PatchedExe
              }
            , Cmd.none
            )

        SelectDocs id ->
            ( { model | selected = id, image = id }, fetchDocs id )

        ImageError ->
            ( { model | image = "Network Addon Mod" }, Cmd.none )

        ChangeExePath path ->
            ( { model | sc4_location = path }, Cmd.none )

        CheckExePath ->
            ( { model | loading = True }, checkExePath model.sc4_location )

        PatchExe ->
            ( { model | loading = True }, patchExe model.sc4_location )

        ChangeLocationOption option ->
            let
                path =
                    case option of
                        "GOG" ->
                            "C:/GOG Games/SimCity 4 Deluxe Edition/Apps"

                        "Disc" ->
                            "C:/Program Files (x86)/Maxis/SimCity 4 Deluxe/Apps"

                        _ ->
                            "C:/Program Files/Steam/steamapps/common/SimCity 4 Deluxe/Apps/SimCity 4.exe"
            in
            ( { model | sc4_location_option = option, sc4_location = path }, Cmd.none )

        GotExePathStatus resp ->
            case resp of
                RemoteData.Success r ->
                    if r.valid then
                        ( { model | state = CheckedExe, loading = False }, Cmd.none )

                    else if String.length r.version < 10 then
                        ( { model | modal = True, loading = False, modal_text = "Your version of SimCity 4 is " ++ r.version ++ ". It must be 1.1.638.0 or higher." }, Cmd.none )

                    else
                        ( { model | modal = True, loading = False, modal_text = "Check your path of SimCity 4, the executable could not be found." }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        GotExePatchStatus resp ->
            case resp of
                RemoteData.Success r ->
                    if r.patched then
                        ( { model | state = PatchedExe, loading = False }, Cmd.none )

                    else
                        ( { model | modal = True, loading = False, modal_text = "Check your path of SimCity 4, the executable could not be found and/or the 4gb+patch failed." }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        ReceiveDocs string ->
            ( { model | docs = RemoteData.unwrap "" identity string }, Cmd.none )

        AddCheckOption ( parent, name ) ->
            ( { model | select_list = (parent ++ "/" ++ name) :: model.select_list |> LExtra.unique }, Cmd.none )

        AddCheckOptionWithChildren ( parent, name, children ) ->
            let
                new_select_list =
                    (parent ++ "/" ++ name)
                        :: model.select_list
                        |> addOptionsRecursively children
                        |> LExtra.unique
            in
            ( { model | select_list = new_select_list }, Cmd.none )

        AddRadioOption ( parent, name, children ) ->
            let
                closest =
                    getClosestParent parent

                remove_list =
                    List.filter
                        (\s -> getClosestParentFromId s == closest)
                        model.select_list

                new_select_list =
                    List.map (\s -> LExtra.remove s model.select_list) remove_list
                        |> List.concat
                        |> List.append [ parent ++ "/" ++ name ]

                add_selected_children =
                    addOptionsRecursively children new_select_list
                        |> LExtra.unique

                other_radio =
                    List.filter
                        (\s -> getClosestParentFromId s == closest)
                        model.select_list
                        |> List.map (\s -> lookupNodeFromId s model.list_installer_options)

                remove_other_radio_children =
                    removeOptionsRecursively (List.map (\i -> unwrapInstallerOption i.children) other_radio |> List.concat) add_selected_children
            in
            ( { model | select_list = remove_other_radio_children }, Cmd.none )

        RemoveOption id ->
            ( { model | select_list = LExtra.remove id model.select_list |> LExtra.unique }, Cmd.none )

        RemoveOptionWithChildren ( parent, name, children ) ->
            let
                new_select_list =
                    removeOptionsRecursively children model.select_list
                        |> LExtra.remove (parent ++ "/" ++ name)
                        |> LExtra.unique
                        |> LExtra.remove parent
            in
            ( { model | select_list = new_select_list }, Cmd.none )

        ReceiveStructure res ->
            case res of
                RemoteData.Success r ->
                    let
                        hide_list =
                            getParents r

                        select_list =
                            getSelected r

                        locked_options =
                            getLocked r
                    in
                    ( { model | installer_options = res, hide_list = hide_list, select_list = select_list, locked_options = locked_options, list_installer_options = r }, Cmd.none )

                _ ->
                    ( { model | installer_options = res }, Cmd.none )


lookupNodeFromId : String -> InstallerOption -> OptionNode
lookupNodeFromId id nodes =
    let
        opts nds =
            unwrapInstallerOption nds
                ++ (List.map (\c -> opts c.children) (unwrapInstallerOption nds) |> List.concat)
    in
    List.filter (\opt -> (opt.parent ++ "/" ++ opt.name) == id) (opts nodes)
        |> List.head
        |> Maybe.withDefault
            { name = "ERROR"
            , radio_check = Locked
            , children = InstallerOption []
            , depth = -100
            , parent = "ERROR"
            }


checkExePath : String -> Cmd Msg
checkExePath path =
    Http.post
        { url = "check_path"
        , body = Http.jsonBody <| Encode.string path
        , expect = Http.expectJson (RemoteData.fromResult >> GotExePathStatus) decodeExeResponse
        }


patchExe : String -> Cmd Msg
patchExe path =
    Http.post
        { url = "patch_exe"
        , body = Http.jsonBody <| Encode.string path
        , expect = Http.expectJson (RemoteData.fromResult >> GotExePatchStatus) decodePatchResponse
        }


type alias ExeResponse =
    { version : String
    , valid : Bool
    }


decodeExeResponse : Decoder ExeResponse
decodeExeResponse =
    Decode.succeed ExeResponse
        |> JsonP.required "version" Decode.string
        |> JsonP.required "valid" Decode.bool


type alias PatchResponse =
    { patched : Bool
    }


decodePatchResponse : Decoder PatchResponse
decodePatchResponse =
    Decode.succeed PatchResponse
        |> JsonP.required "patched" Decode.bool


addOptionsRecursively : List OptionNode -> List String -> List String
addOptionsRecursively options current =
    let
        no_radio =
            List.filter (\i -> i.radio_check /= Radio) options
    in
    List.append current
        (List.map
            (\i -> i.parent ++ "/" ++ i.name)
            no_radio
            ++ (List.map (\c -> addOptionsRecursively (unwrapInstallerOption c.children) current) no_radio |> List.concat)
        )


removeOptionsRecursively : List OptionNode -> List String -> List String
removeOptionsRecursively options current =
    let
        ( radio, no_radio ) =
            List.partition (\i -> i.radio_check == Radio || i.radio_check == RadioChecked || i.radio_check == RadioFolder) options

        -- remove_radio_children =
        --     List.filter (\i -> i.radio_check == RadioChecked && List.length (unwrapInstallerOption i.children) > 0) radio
        --         |> List.map (\c -> getId c.children)
        --         |> List.concat
        -- _ =
        --     Debug.log "" remove_radio_children
        to_remove =
            List.map (\i -> i.parent ++ "/" ++ i.name) no_radio
                |> List.append (List.map (\c -> getId c.children) no_radio |> List.concat)

        -- |> List.append remove_radio_children
    in
    List.filter (\c -> not <| List.member c to_remove) current


getClosestParent : String -> String
getClosestParent s =
    String.split "/" s |> List.reverse |> List.head |> Maybe.withDefault ""


getClosestParentFromId : String -> String
getClosestParentFromId s =
    String.split "/" s |> List.take (List.length (String.split "/" s) - 1) |> List.reverse |> List.head |> Maybe.withDefault ""


getParents : InstallerOption -> List String
getParents option =
    LExtra.unique
        (List.map .parent (unwrapInstallerOption option) |> List.filter (\item -> List.length (String.split "/" item) > 1))
        ++ (List.map (\c -> getParents c.children) (unwrapInstallerOption option) |> List.concat)


getId : InstallerOption -> List String
getId option =
    LExtra.unique
        (List.map (\i -> i.parent ++ "/" ++ i.name) (unwrapInstallerOption option))
        ++ (List.map (\c -> getId c.children) (unwrapInstallerOption option) |> List.concat)


getSelected : InstallerOption -> List String
getSelected option =
    LExtra.unique <|
        (List.map (\i -> i.parent ++ "/" ++ i.name) <| List.filter (\i -> i.radio_check == Checked || i.radio_check == RadioChecked) (unwrapInstallerOption option))
            ++ (List.concat <| List.map (\l -> getSelected l.children) (unwrapInstallerOption option))


getLocked : InstallerOption -> List String
getLocked option =
    LExtra.unique <|
        (List.map (\i -> i.parent ++ "/" ++ i.name) <| List.filter (\i -> i.radio_check == Locked) (unwrapInstallerOption option))
            ++ (List.concat <| List.map (\l -> getLocked l.children) (unwrapInstallerOption option))


view : Model -> Browser.Document Msg
view model =
    let
        innerHTML =
            displayInstaller model

        version =
            model.flags.rust_version
    in
    { title = "Network Addon Mod Installer v" ++ version
    , body =
        [ innerHTML ]
    }


displayInstaller : Model -> Html Msg
displayInstaller model =
    div [ class "application-area" ]
        [ div [ style "padding" "15px" ]
            [ h3 [ class "title is-3", style "margin-bottom" "0px" ] [ text <| "Network Addon Mod Installer v" ++ model.flags.rust_version ]
            , br [] []
            , if model.flags.windows == "true" then
                div []
                    [ div [ class "columns" ]
                        [ div [ class "column is-narrow" ]
                            [ label [ class "label" ] [ text "Steam" ]
                            , input
                                [ onClick (ChangeLocationOption "Steam")
                                , disabled (model.state /= TCAccept)
                                , style "margin-left" "35%"
                                , type_ "radio"
                                , name "location"
                                , checked ("Steam" == model.sc4_location_option)
                                ]
                                []
                            ]
                        , div [ class "column is-narrow" ]
                            [ label [ class "label" ] [ text "GOG" ]
                            , input
                                [ onClick (ChangeLocationOption "GOG")
                                , disabled (model.state /= TCAccept)
                                , style "margin-left" "33%"
                                , type_ "radio"
                                , name "location"
                                , checked ("GOG" == model.sc4_location_option)
                                ]
                                []
                            ]
                        , div [ class "column is-narrow" ]
                            [ label [ class "label" ] [ text "Disc" ]
                            , input
                                [ onClick (ChangeLocationOption "Disc")
                                , disabled (model.state /= TCAccept)
                                , style "margin-left" "33%"
                                , type_ "radio"
                                , name "location"
                                , checked ("Disc" == model.sc4_location_option)
                                ]
                                []
                            ]
                        ]
                    , div [ class "buttons has-addons columns" ]
                        [ div [ class "column is-5" ]
                            [ input
                                [ type_ "text"
                                , class "input"
                                , value model.sc4_location
                                , onInput ChangeExePath
                                , disabled (model.state /= TCAccept)
                                ]
                                []
                            ]
                        , div [ class "column is-narrow" ]
                            [ button
                                [ class "button is-success"
                                , class <|
                                    if model.loading then
                                        "is-loading"

                                    else
                                        ""
                                , style "margin-bottom" "0rem"
                                , onClick CheckExePath
                                , disabled (model.state /= TCAccept)
                                ]
                                [ text "Check SimCity 4 Executable Location" ]
                            ]
                        ]
                    , div [ class "" ]
                        [ button
                            [ class "button is-link"
                            , style "margin-bottom" "0rem"
                            , class <|
                                if model.loading then
                                    "is-loading"

                                else
                                    ""
                            , onClick PatchExe
                            , disabled (model.state /= CheckedExe)
                            ]
                            [ text "Patch SimCity 4 Executable at Location" ]
                        ]
                    ]

              else
                div [] []
            , br [] []
            , br [] []
            , div [ class "columns" ]
                [ case model.installer_options of
                    RemoteData.Success opts ->
                        div [ class "is-half", style "max-height" "70vh", style "overflow-y" "auto", style "min-height" "70vh", style "width" "50vw" ]
                            (List.concat <|
                                List.map (displayOptions model) <|
                                    unwrapInstallerOption opts
                            )

                    RemoteData.Loading ->
                        div [] [ text "Loading..." ]

                    RemoteData.NotAsked ->
                        div [] [ text "Not Asked" ]

                    RemoteData.Failure f ->
                        let
                            err =
                                case f of
                                    Http.NetworkError ->
                                        "Network Error"

                                    Http.BadBody bb ->
                                        "Bad Body: " ++ bb

                                    Http.BadStatus bs ->
                                        "Bad Status: " ++ String.fromInt bs

                                    Http.BadUrl bu ->
                                        "Bad Url: " ++ bu

                                    Http.Timeout ->
                                        "Timeout"
                        in
                        div [] [ text err ]
                , div [ class "is-half tile is-ancestor", style "max-height" "70vh", style "overflow-y" "auto", style "min-height" "70vh", style "width" "50vw" ]
                    [ section [ class "section" ]
                        [ div [ class "tile is-vertical" ]
                          -- [ div [ class "tile is-child", style "min-height" "30vh" ] [ p [] [ text model.docs ] ]
                          -- , div [ class "tile is-child" ]
                          --     [ img
                          --         [ src ("images/" ++ (String.replace "top/" "" model.image |> String.replace "/" "%2F") ++ ".png")
                          --         , on "error" (Decode.succeed ImageError)
                          --         ]
                          --         []
                          --     ]
                          -- ]
                          <|
                            List.map (\item -> p [] [ text item ]) <|
                                List.sort model.select_list
                        ]
                    ]
                ]
            ]
        , if model.modal then
            div [ class "modal is-active" ]
                [ div [ class "modal-background" ]
                    []
                , div [ class "modal-card" ]
                    [ section [ class "modal-card-body" ]
                        [ text model.modal_text
                        ]
                    , button [ class "modal-close is-large", onClick HideModal ] []
                    ]
                ]

          else
            div [] []
        , if not model.tc then
            div [ class "modal is-active" ]
                [ div [ class "modal-background" ]
                    []
                , div [ class "modal-card" ]
                    [ section [ class "modal-card-body" ]
                        [ div [] tcText
                        , button [ class "button is-success", onClick AcceptTC ] [ text "I agree" ]
                        ]
                    ]
                ]

          else
            div [] []
        ]


displayOptions : Model -> OptionNode -> List (Html Msg)
displayOptions model option =
    let
        tabs =
            String.concat <|
                List.map (\_ -> "       ") <|
                    List.range 0 option.depth

        name_ =
            option.name
    in
    [ div
        [ style "display" <|
            if List.member option.parent model.hide_list then
                "none"

            else
                "inherit"
        ]
        [ tr
            []
            [ td [ style "white-space" "pre", style "padding-right" "10px" ]
                [ text tabs ]
            , td [ style "white-space" "pre", style "padding-right" "20px" ] <|
                case option.children of
                    InstallerOption list ->
                        if List.length list > 0 then
                            if List.member (option.parent ++ "/" ++ option.name) model.hide_list then
                                [ button
                                    [ class "button is-small is-outlined"
                                    , disabled (model.state /= PatchedExe)
                                    , onClick (ToggleExpand Expand (option.parent ++ "/" ++ option.name))
                                    ]
                                    [ text "＋" ]
                                ]

                            else
                                [ button
                                    [ class "button is-small is-outlined"
                                    , disabled (model.state /= PatchedExe)
                                    , onClick (ToggleExpand Collapse (option.parent ++ "/" ++ option.name))
                                    ]
                                    [ text "－" ]
                                ]

                        else
                            [ button [ class "button is-small is-outlined invisible no-text" ] [ text "＋" ] ]
            , case ( option.radio_check, List.member (option.parent ++ "/" ++ option.name) model.select_list ) of
                ( Radio, False ) ->
                    radioUnchecked model option

                ( Radio, True ) ->
                    radioChecked model option

                ( RadioChecked, False ) ->
                    radioUnchecked model option

                ( RadioChecked, True ) ->
                    radioChecked model option

                ( RadioFolder, _ ) ->
                    radioFolder option

                ( Checked, False ) ->
                    uncheckedBox model option

                ( Checked, True ) ->
                    checkedBox model option

                ( Unchecked, False ) ->
                    uncheckedBox model option

                ( Unchecked, True ) ->
                    checkedBox model option

                ( Locked, _ ) ->
                    locked option

                ( ParentLocked, _ ) ->
                    parentLocked model option
            , td [ style "vertical-align" "middle", style "padding-left" "20px" ]
                [ button
                    [ class "button is-small"
                    , onClick (SelectDocs (option.parent ++ "/" ++ option.name))
                    , name option.parent
                    , class
                        (if model.selected == (option.parent ++ "/" ++ option.name) then
                            "is-black"

                         else
                            "invisible"
                        )
                    ]
                    [ text <| name_ ]
                ]
            ]
        , br [] []
        , div [] <|
            List.concat <|
                case option.children of
                    InstallerOption list ->
                        List.map (displayOptions model) list
        ]
    ]


radioUnchecked : Model -> OptionNode -> Html Msg
radioUnchecked model option =
    td [ style "vertical-align" "middle" ]
        [ input
            [ style "transform" "scale(1.5)"
            , type_ "radio"
            , disabled (model.state /= PatchedExe)
            , name <| String.fromInt option.depth ++ "_" ++ option.parent
            , onClick (AddRadioOption ( option.parent, option.name, unwrapInstallerOption option.children ))
            ]
            []
        ]


radioChecked : Model -> OptionNode -> Html Msg
radioChecked model option =
    td [ style "vertical-align" "middle" ]
        [ input
            [ style "transform" "scale(1.5)"
            , type_ "radio"
            , disabled (model.state /= PatchedExe)
            , checked True
            , name <| String.fromInt option.depth ++ "_" ++ option.parent
            ]
            []
        ]


radioFolder : OptionNode -> Html Msg
radioFolder _ =
    td [ style "vertical-align" "middle" ]
        [ input [ style "transform" "scale(1.5)", type_ "radio", disabled True ] [] ]


checkedBox : Model -> OptionNode -> Html Msg
checkedBox model option =
    let
        children =
            unwrapInstallerOption option.children
    in
    if List.length children > 0 then
        td [ style "vertical-align" "middle" ]
            [ input
                [ style "transform" "scale(1.5)"
                , type_ "checkbox"
                , checked True
                , disabled (model.state /= PatchedExe)
                , onClick (RemoveOptionWithChildren ( option.parent, option.name, children ))
                ]
                []
            ]

    else
        td [ style "vertical-align" "middle" ]
            [ input
                [ style "transform" "scale(1.5)"
                , type_ "checkbox"
                , checked True
                , disabled (model.state /= PatchedExe)
                , onClick (RemoveOption (option.parent ++ "/" ++ option.name))
                ]
                []
            ]


uncheckedBox : Model -> OptionNode -> Html Msg
uncheckedBox model option =
    let
        children =
            unwrapInstallerOption option.children
    in
    if List.length children > 0 then
        td [ style "vertical-align" "middle" ]
            [ input
                [ style "transform" "scale(1.5)"
                , type_ "checkbox"
                , disabled (model.state /= PatchedExe)
                , onClick
                    (AddCheckOptionWithChildren
                        ( option.parent
                        , option.name
                        , children
                        )
                    )
                ]
                []
            ]

    else
        td [ style "vertical-align" "middle" ]
            [ input
                [ style "transform" "scale(1.5)"
                , type_ "checkbox"
                , onClick (AddCheckOption ( option.parent, option.name ))
                ]
                []
            ]


locked : OptionNode -> Html Msg
locked _ =
    td [ style "vertical-align" "middle" ]
        [ input [ style "transform" "scale(1.5)", type_ "checkbox", checked True, disabled True ] [] ]


parentLocked : Model -> OptionNode -> Html Msg
parentLocked model option =
    let
        closest_parent =
            getClosestParent option.parent

        chkd =
            if List.member closest_parent (List.map getClosestParent model.select_list) then
                checked True

            else
                class ""
    in
    td [ style "vertical-align" "middle" ]
        [ input [ style "transform" "scale(1.5)", type_ "checkbox", chkd, disabled True ] [] ]


unwrapInstallerOption : InstallerOption -> List OptionNode
unwrapInstallerOption option =
    case option of
        InstallerOption l ->
            l


tcText : List (Html Msg)
tcText =
    [ p [] [ text "Welcome to the Network Addon Mod 37 installer application! Please read the following carefully, then select 'I agree with these conditions' to continue." ]
    , p [] [ text "---------------------------------------------------------------------------------------------" ]
    , p [] [ text "Users download, install, and run this software completely and solely at their own risk. Maxis, Electronic Arts,the creators, and its individual contributors are not responsible for any errors, crashes, problems, or any other issue that you may have if you have downloaded and applied this software to your game. Players should also expect that any future patches and/or expansion packs and SimCityscape may not function properly with the game if you have downloaded this  software and applied it to your game. The use of this software, the information\n within, and the Network Addon Mod is conditional upon the acceptance of this disclaimer and all that is within this software." ]
    , p [] [ text "---------------------------------------------------------------------------------------------" ]
    ]
