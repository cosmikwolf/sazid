use bat::assets::HighlightingAssets;
use bat::config::Config;
use bat::style::StyleComponents;

#[derive(Clone)]
pub struct BatRenderer<'a> {
    config: Config<'a>,
}

impl<'a> Default for BatRenderer<'a> {
    fn default() -> Self {
        BatRenderer::new(80)
    }
}

impl<'a> std::fmt::Debug for BatRenderer<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatRenderer").finish()
    }
}

impl<'a> BatRenderer<'a> {
    // const ASSETS: HighlightingAssets = HighlightingAssets::from_binary();

    fn new(term_width: usize) -> Self {
        let style_components = StyleComponents::new(&[
      //StyleComponent::Header,
      //StyleComponent::Grid,
      //StyleComponent::LineNumbers,
      //StyleComponent::Changes,
      //StyleComponent::Rule,
      //StyleComponent::Default,
      //StyleComponent::Snip,
      //StyleComponents::plain,
    ]);
        let config: Config<'static> = Config {
            colored_output: true,
            language: Some("markdown"),
            style_components,
            show_nonprintable: false,
            tab_width: 2,
            wrapping_mode: bat::WrappingMode::NoWrapping(false),
            use_italic_text: true,
            term_width,
            paging_mode: bat::PagingMode::Never,
            true_color: true,
            use_custom_assets: true,
            ..Default::default()
        };
        let _buffer: Vec<u8> = Vec::new();
        BatRenderer { config }
    }
}
